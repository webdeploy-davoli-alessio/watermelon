use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::sync::oneshot;
use watermelon_proto::{ServerMessage, Subject};

use crate::{client::ClientClosedError, core::Client};

#[derive(Debug)]
pub(crate) struct MultiplexedSubscription {
    subscription: Option<Inner>,
}

#[derive(Debug)]
struct Inner {
    reply_subject: Subject,
    receiver: oneshot::Receiver<ServerMessage>,
    client: Client,
}

impl MultiplexedSubscription {
    pub(crate) fn new(
        reply_subject: Subject,
        receiver: oneshot::Receiver<ServerMessage>,
        client: Client,
    ) -> Self {
        Self {
            subscription: Some(Inner {
                reply_subject,
                receiver,
                client,
            }),
        }
    }
}

impl Future for MultiplexedSubscription {
    type Output = Result<ServerMessage, ClientClosedError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let subscription = self
            .subscription
            .as_mut()
            .expect("MultiplexedSubscription polled after completing");

        match Pin::new(&mut subscription.receiver).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.subscription = None;
                Poll::Ready(result.map_err(|_| ClientClosedError))
            }
        }
    }
}

impl Drop for MultiplexedSubscription {
    fn drop(&mut self) {
        let Some(subscription) = self.subscription.take() else {
            return;
        };

        subscription
            .client
            .lazy_unsubscribe_multiplexed(subscription.reply_subject);
    }
}
