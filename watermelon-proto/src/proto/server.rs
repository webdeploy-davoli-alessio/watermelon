use alloc::boxed::Box;

use crate::{error::ServerError, message::ServerMessage, ServerInfo};

#[derive(Debug, PartialEq, Eq)]
pub enum ServerOp {
    Info { info: Box<ServerInfo> },
    Message { message: ServerMessage },
    Success,
    Error { error: ServerError },
    Ping,
    Pong,
}
