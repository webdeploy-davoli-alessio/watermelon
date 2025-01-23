use core::fmt::{self, Write as _};
#[cfg(feature = "std")]
use std::io;

use bytes::Bytes;

use crate::headers::HeaderMap;
use crate::MessageBase;

pub use self::framed::FramedEncoder;
pub use self::stream::StreamEncoder;

use super::ClientOp;

mod framed;
mod stream;

pub(super) trait FrameEncoder {
    fn small_write(&mut self, buf: &[u8]);

    fn write<B>(&mut self, buf: B)
    where
        B: Into<Bytes> + AsRef<[u8]>,
    {
        self.small_write(buf.as_ref());
    }

    fn small_fmt_writer(&mut self) -> SmallFmtWriter<'_, Self> {
        SmallFmtWriter(self)
    }

    #[cfg(feature = "std")]
    fn small_io_writer(&mut self) -> SmallIoWriter<'_, Self> {
        SmallIoWriter(self)
    }
}

pub(super) struct SmallFmtWriter<'a, E: ?Sized>(&'a mut E);

impl<E> fmt::Write for SmallFmtWriter<'_, E>
where
    E: FrameEncoder,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.small_write(s.as_bytes());
        Ok(())
    }
}

#[cfg(feature = "std")]
pub(super) struct SmallIoWriter<'a, E: ?Sized>(&'a mut E);

#[cfg(feature = "std")]
impl<E> io::Write for SmallIoWriter<'_, E>
where
    E: FrameEncoder,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.small_write(buf);
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.small_write(buf);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode<E: FrameEncoder>(encoder: &mut E, item: &ClientOp) {
    macro_rules! small_write {
        ($dst:expr) => {
            write!(encoder.small_fmt_writer(), $dst).expect("do small write to Connection");
        };
    }

    match item {
        ClientOp::Publish { message } => {
            let MessageBase {
                subject,
                reply_subject,
                headers,
                payload,
            } = &message;
            let verb = if headers.is_empty() { "PUB" } else { "HPUB" };

            small_write!("{verb} {subject} ");

            if let Some(reply_subject) = reply_subject {
                small_write!("{reply_subject} ");
            }

            if headers.is_empty() {
                let payload_len = payload.len();
                small_write!("{payload_len}\r\n");
            } else {
                let headers_len = encode_headers(headers).fold(0, |len, s| len + s.len());

                let total_len = headers_len + payload.len();
                small_write!("{headers_len} {total_len}\r\n");

                encode_headers(headers).for_each(|s| {
                    encoder.small_write(s.as_bytes());
                });
            }

            encoder.write(IntoBytes(payload));
            encoder.small_write(b"\r\n");
        }
        ClientOp::Subscribe {
            id,
            subject,
            queue_group,
        } => match queue_group {
            Some(queue_group) => {
                small_write!("SUB {subject} {queue_group} {id}\r\n");
            }
            None => {
                small_write!("SUB {subject} {id}\r\n");
            }
        },
        ClientOp::Unsubscribe { id, max_messages } => match max_messages {
            Some(max_messages) => {
                small_write!("UNSUB {id} {max_messages}\r\n");
            }
            None => {
                small_write!("UNSUB {id}\r\n");
            }
        },
        ClientOp::Connect { connect } => {
            encoder.small_write(b"CONNECT ");
            #[cfg(feature = "std")]
            serde_json::to_writer(encoder.small_io_writer(), &connect)
                .expect("serialize `Connect`");
            #[cfg(not(feature = "std"))]
            encoder.write(serde_json::to_vec(&connect).expect("serialize `Connect`"));
            encoder.small_write(b"\r\n");
        }
        ClientOp::Ping => {
            encoder.small_write(b"PING\r\n");
        }
        ClientOp::Pong => {
            encoder.small_write(b"PONG\r\n");
        }
    }
}

struct IntoBytes<'a>(&'a Bytes);

impl<'a> From<IntoBytes<'a>> for Bytes {
    fn from(value: IntoBytes<'a>) -> Self {
        Bytes::clone(value.0)
    }
}

impl AsRef<[u8]> for IntoBytes<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

fn encode_headers(headers: &HeaderMap) -> impl Iterator<Item = &'_ str> {
    let head = ["NATS/1.0\r\n"];
    let headers = headers.iter().flat_map(|(name, values)| {
        values.flat_map(|value| [name.as_str(), ": ", value.as_str(), "\r\n"])
    });
    let footer = ["\r\n"];

    head.into_iter().chain(headers).chain(footer)
}
