pub use self::client::ClientOp;
pub use self::decoder::{decode_frame, StreamDecoder};
pub use self::encoder::{FramedEncoder, StreamEncoder};
pub use self::server::ServerOp;

mod client;
mod decoder;
mod encoder;
mod server;

pub mod error {
    pub use super::decoder::{DecoderError, FrameDecoderError};
}
