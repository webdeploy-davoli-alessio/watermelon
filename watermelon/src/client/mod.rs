use std::{fmt::Write, num::NonZeroU64, process::abort, sync::Arc, time::Duration};
#[cfg(test)]
use std::{
    net::{IpAddr, Ipv4Addr},
    num::{NonZeroU16, NonZeroU32},
};

use arc_swap::ArcSwap;
use bytes::Bytes;
use rand::RngCore;
use tokio::{
    sync::{
        mpsc::{self, error::TrySendError, Permit},
        oneshot,
    },
    task::JoinHandle,
    time::{interval, MissedTickBehavior},
};
use watermelon_mini::ConnectError;
#[cfg(test)]
use watermelon_proto::NonStandardServerInfo;
use watermelon_proto::{
    headers::HeaderMap, QueueGroup, ServerAddr, ServerInfo, Subject, SubscriptionId,
};

pub use self::builder::{ClientBuilder, Echo};
pub use self::commands::{
    ClientPublish, ClientRequest, DoClientPublish, DoClientRequest, DoOwnedClientPublish,
    DoOwnedClientRequest, OwnedClientPublish, OwnedClientRequest, Publish, PublishBuilder, Request,
    RequestBuilder, ResponseError, ResponseFut,
};
pub use self::jetstream::{
    AckPolicy, Compression, Consumer, ConsumerBatch, ConsumerConfig, ConsumerDurability,
    ConsumerSpecificConfig, ConsumerStorage, ConsumerStream, ConsumerStreamError, Consumers,
    DeliverPolicy, DiscardPolicy, JetstreamClient, JetstreamError, JetstreamError2,
    JetstreamErrorCode, ReplayPolicy, RetentionPolicy, Storage, Stream, StreamConfig, StreamState,
    Streams,
};
pub use self::quick_info::QuickInfo;
pub(crate) use self::quick_info::RawQuickInfo;
#[cfg(test)]
use self::tests::TestHandler;
use crate::{
    atomic::{AtomicU64, Ordering},
    core::{MultiplexedSubscription, Subscription},
    handler::{
        Handler, HandlerCommand, HandlerOutput, RecycledHandler, MULTIPLEXED_SUBSCRIPTION_ID,
    },
};

mod builder;
mod commands;
mod jetstream;
mod quick_info;
#[cfg(test)]
pub(crate) mod tests;

#[cfg(feature = "from-env")]
pub(super) mod from_env;

const CLIENT_OP_CHANNEL_SIZE: usize = 512;
const SUBSCRIPTION_CHANNEL_SIZE: usize = 256;
const RECONNECT_DELAY: Duration = Duration::from_secs(10);

/// A NATS client
///
/// `Client` is a `Clone`able handle to a NATS connection.
/// If the connection is lost, the client will automatically reconnect and
/// resume any currently open subscriptions.
#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

#[derive(Debug)]
struct ClientInner {
    sender: mpsc::Sender<HandlerCommand>,
    info: Arc<ArcSwap<ServerInfo>>,
    quick_info: Arc<RawQuickInfo>,
    multiplexed_subscription_prefix: Subject,
    next_subscription_id: AtomicU64,
    inbox_prefix: Subject,
    default_response_timeout: Duration,
    handler: JoinHandle<()>,
}

/// An error encountered while trying to publish a command to a closed [`Client`]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("client closed")]
pub struct ClientClosedError;

#[derive(Debug, thiserror::Error)]
#[error("try command error")]
pub enum TryCommandError {
    /// The client's internal buffer is currently full
    #[error("buffer full")]
    BufferFull,
    /// The client has been closed via [`Client::close`]
    #[error("client closed")]
    Closed(#[source] ClientClosedError),
}

impl Client {
    /// Construct a new client
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub(super) async fn connect(
        addr: ServerAddr,
        builder: ClientBuilder,
    ) -> Result<Self, ConnectError> {
        let (sender, receiver) = mpsc::channel(CLIENT_OP_CHANNEL_SIZE);

        let quick_info = Arc::new(RawQuickInfo::new());
        let handle = RecycledHandler::new(receiver, Arc::clone(&quick_info), &builder);
        let handle = Handler::connect(&addr, &builder, handle)
            .await
            .map_err(|(err, _recycle)| err)?;
        let info = handle.info().clone();
        let multiplexed_subscription_prefix = handle.multiplexed_subscription_prefix().clone();
        let inbox_prefix = builder.inbox_prefix.clone();
        let default_response_timeout = builder.default_response_timeout;

        let handler = tokio::spawn(async move {
            let mut handle = handle;

            loop {
                match (&mut handle).await {
                    HandlerOutput::ServerError | HandlerOutput::Disconnected => {
                        let mut recycle = handle.recycle().await;

                        let mut interval = interval(RECONNECT_DELAY);
                        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

                        loop {
                            interval.tick().await;

                            match Handler::connect(&addr, &builder, recycle).await {
                                Ok(new_handle) => {
                                    handle = new_handle;
                                    break;
                                }
                                Err((_err, prev_recycle)) => recycle = prev_recycle,
                            }
                        }
                    }
                    HandlerOutput::UnexpectedState => {
                        // Retry and hope for the best
                    }
                    HandlerOutput::Closed => break,
                }
            }
        });

        Ok(Self {
            inner: Arc::new(ClientInner {
                info,
                sender,
                quick_info,
                multiplexed_subscription_prefix,
                next_subscription_id: AtomicU64::new(u64::from(MULTIPLEXED_SUBSCRIPTION_ID) + 1),
                inbox_prefix,
                default_response_timeout,
                handler,
            }),
        })
    }

    #[cfg(test)]
    pub(crate) fn test(client_to_handler_chan_size: usize) -> (Self, TestHandler) {
        let builder = Self::builder();
        let (sender, receiver) = mpsc::channel(client_to_handler_chan_size);
        let info = Arc::new(ArcSwap::new(Arc::from(ServerInfo {
            id: "1234".to_owned(),
            name: "watermelon-test".to_owned(),
            version: "2.10.17".to_owned(),
            go_version: "1.22.5".to_owned(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: NonZeroU16::new(4222).unwrap(),
            supports_headers: true,
            max_payload: NonZeroU32::new(1024 * 1024).unwrap(),
            protocol_version: 2,
            client_id: Some(1),
            auth_required: false,
            tls_required: false,
            tls_verify: false,
            tls_available: false,
            connect_urls: Vec::new(),
            websocket_connect_urls: Vec::new(),
            lame_duck_mode: false,
            git_commit: None,
            supports_jetstream: true,
            ip: None,
            client_ip: None,
            nonce: None,
            cluster_name: None,
            domain: None,

            non_standard: NonStandardServerInfo::default(),
        })));
        let quick_info = Arc::new(RawQuickInfo::new());
        let multiplexed_subscription_prefix = create_inbox_subject(&builder.inbox_prefix);

        let this = Self {
            inner: Arc::new(ClientInner {
                sender,
                info: Arc::clone(&info),
                quick_info: Arc::clone(&quick_info),
                multiplexed_subscription_prefix,
                next_subscription_id: AtomicU64::new(1),
                inbox_prefix: builder.inbox_prefix,
                default_response_timeout: builder.default_response_timeout,
                handler: tokio::spawn(async move {}),
            }),
        };
        let handler = TestHandler {
            receiver,
            _info: info,
            quick_info,
        };
        (this, handler)
    }

    /// Publish a new message to the NATS server
    ///
    /// Consider calling [`Publish::client`] instead if you already have
    /// a [`Publish`] instance.
    #[must_use]
    pub fn publish(&self, subject: Subject) -> ClientPublish {
        ClientPublish::build(self, subject)
    }

    /// Publish a new message to the NATS server
    ///
    /// Consider calling [`Request::client`] instead if you already have
    /// a [`Request`] instance.
    #[must_use]
    pub fn request(&self, subject: Subject) -> ClientRequest {
        ClientRequest::build(self, subject)
    }

    /// Publish a new message to the NATS server, taking ownership of this client
    ///
    /// When possible consider using [`Client::publish`] instead.
    ///
    /// Consider calling [`Publish::client_owned`] instead if you already have
    /// a [`Publish`] instance.
    #[must_use]
    pub fn publish_owned(self, subject: Subject) -> OwnedClientPublish {
        OwnedClientPublish::build(self, subject)
    }

    /// Publish a new message to the NATS server, taking ownership of this client
    ///
    /// When possible consider using [`Client::request`] instead.
    ///
    /// Consider calling [`Request::client_owned`] instead if you already have
    /// a [`Request`] instance.
    #[must_use]
    pub fn request_owned(self, subject: Subject) -> OwnedClientRequest {
        OwnedClientRequest::build(self, subject)
    }

    /// Subscribe to the given filter subject
    ///
    /// Create a new subscription with the NATS server and ask for all
    /// messages matching the given `filter_subject` to be delivered
    /// to the client.
    ///
    /// If `queue_group` is provided and multiple clients subscribe with
    /// the same [`QueueGroup`] value, the NATS server will try to deliver
    /// these messages to only one of the clients.
    ///
    /// If the client was built with [`Echo::Allow`], then messages
    /// published by this same client may be received by this subscription.
    ///
    /// # Errors
    ///
    /// This returns an error if the connection with the client is closed.
    pub async fn subscribe(
        &self,
        filter_subject: Subject,
        queue_group: Option<QueueGroup>,
    ) -> Result<Subscription, ClientClosedError> {
        let permit = self
            .inner
            .sender
            .reserve()
            .await
            .map_err(|_| ClientClosedError)?;

        Ok(self.do_subscribe(permit, filter_subject, queue_group))
    }

    pub(crate) fn try_subscribe(
        &self,
        filter_subject: Subject,
        queue_group: Option<QueueGroup>,
    ) -> Result<Subscription, TryCommandError> {
        let permit = self
            .inner
            .sender
            .try_reserve()
            .map_err(|_| TryCommandError::BufferFull)?;

        Ok(self.do_subscribe(permit, filter_subject, queue_group))
    }

    fn do_subscribe(
        &self,
        permit: Permit<'_, HandlerCommand>,
        filter_subject: Subject,
        queue_group: Option<QueueGroup>,
    ) -> Subscription {
        let id = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::AcqRel)
            .into();
        if id == SubscriptionId::MAX {
            abort();
        }
        let (sender, receiver) = mpsc::channel(SUBSCRIPTION_CHANNEL_SIZE);

        permit.send(HandlerCommand::Subscribe {
            id,
            subject: filter_subject,
            queue_group,
            messages: sender,
        });
        Subscription::new(id, self.clone(), receiver)
    }

    pub(super) async fn multiplexed_request(
        &self,
        subject: Subject,
        headers: HeaderMap,
        payload: Bytes,
    ) -> Result<MultiplexedSubscription, ClientClosedError> {
        let permit = self
            .inner
            .sender
            .reserve()
            .await
            .map_err(|_| ClientClosedError)?;

        Ok(self.do_multiplexed_request(permit, subject, headers, payload))
    }

    pub(super) fn try_multiplexed_request(
        &self,
        subject: Subject,
        headers: HeaderMap,
        payload: Bytes,
    ) -> Result<MultiplexedSubscription, TryCommandError> {
        let permit = self
            .inner
            .sender
            .try_reserve()
            .map_err(|_| TryCommandError::BufferFull)?;

        Ok(self.do_multiplexed_request(permit, subject, headers, payload))
    }

    fn do_multiplexed_request(
        &self,
        permit: Permit<'_, HandlerCommand>,
        subject: Subject,
        headers: HeaderMap,
        payload: Bytes,
    ) -> MultiplexedSubscription {
        let (sender, receiver) = oneshot::channel();

        let reply_subject = create_inbox_subject(&self.inner.multiplexed_subscription_prefix);

        permit.send(HandlerCommand::RequestMultiplexed {
            subject,
            reply_subject: reply_subject.clone(),
            headers,
            payload,
            reply: sender,
        });
        MultiplexedSubscription::new(reply_subject, receiver, self.clone())
    }

    /// Get the last [`ServerInfo`] sent by the server
    ///
    /// Consider calling [`Client::quick_info`] if you only need
    /// information about Lame Duck Mode.
    #[must_use]
    pub fn server_info(&self) -> Arc<ServerInfo> {
        self.inner.info.load_full()
    }

    /// Get information about the client
    #[must_use]
    pub fn quick_info(&self) -> QuickInfo {
        self.inner.quick_info.get()
    }

    pub(crate) fn create_inbox_subject(&self) -> Subject {
        create_inbox_subject(&self.inner.inbox_prefix)
    }

    pub(crate) fn default_response_timeout(&self) -> Duration {
        self.inner.default_response_timeout
    }

    pub(crate) fn lazy_unsubscribe_multiplexed(&self, reply_subject: Subject) {
        if self
            .try_enqueue_command(HandlerCommand::UnsubscribeMultiplexed { reply_subject })
            .is_ok()
        {
            return;
        }

        self.inner.quick_info.store_is_failed_unsubscribe(true);
    }

    pub(crate) async fn unsubscribe(
        &self,
        id: SubscriptionId,
        max_messages: Option<NonZeroU64>,
    ) -> Result<(), ClientClosedError> {
        self.enqueue_command(HandlerCommand::Unsubscribe { id, max_messages })
            .await
    }

    pub(crate) fn lazy_unsubscribe(&self, id: SubscriptionId, max_messages: Option<NonZeroU64>) {
        if self
            .try_enqueue_command(HandlerCommand::Unsubscribe { id, max_messages })
            .is_ok()
        {
            return;
        }

        self.inner.quick_info.store_is_failed_unsubscribe(true);
    }

    pub(super) async fn enqueue_command(
        &self,
        cmd: HandlerCommand,
    ) -> Result<(), ClientClosedError> {
        self.inner
            .sender
            .send(cmd)
            .await
            .map_err(|_| ClientClosedError)
    }

    pub(super) fn try_enqueue_command(&self, cmd: HandlerCommand) -> Result<(), TryCommandError> {
        self.inner
            .sender
            .try_send(cmd)
            .map_err(TryCommandError::from_try_send_error)
    }

    /// Close this client, waiting for any remaining buffered messages to be processed first
    ///
    /// Attempts to send commands to the NATS server after this method has been called will
    /// result into a [`ClientClosedError`] error.
    pub async fn close(&self) {
        let (sender, receiver) = oneshot::channel();
        if self
            .enqueue_command(HandlerCommand::Close(sender))
            .await
            .is_err()
        {
            return;
        }

        let _ = receiver.await;
    }
}

impl Drop for ClientInner {
    fn drop(&mut self) {
        self.handler.abort();
    }
}

impl TryCommandError {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "this is an auxiliary conversion function"
    )]
    pub(crate) fn from_try_send_error<T>(err: TrySendError<T>) -> Self {
        match err {
            TrySendError::Full(_) => Self::BufferFull,
            TrySendError::Closed(_) => Self::Closed(ClientClosedError),
        }
    }
}

pub(crate) fn create_inbox_subject(prefix: &Subject) -> Subject {
    let mut suffix = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut suffix);

    let mut subject = String::with_capacity(prefix.len() + ".".len() + (suffix.len() * 2));
    write!(&mut subject, "{}.{:x}", prefix, u128::from_ne_bytes(suffix)).unwrap();

    Subject::from_dangerous_value(subject.into())
}
