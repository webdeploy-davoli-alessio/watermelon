use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::mpsc;
use watermelon_proto::ServerInfo;

use crate::{client::RawQuickInfo, handler::HandlerCommand};

#[derive(Debug)]
pub(crate) struct TestHandler {
    pub(crate) receiver: mpsc::Receiver<HandlerCommand>,
    pub(crate) _info: Arc<ArcSwap<ServerInfo>>,
    pub(crate) quick_info: Arc<RawQuickInfo>,
}
