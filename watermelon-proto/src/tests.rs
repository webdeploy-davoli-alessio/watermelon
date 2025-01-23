use bytes::{Buf, Bytes};

pub(crate) trait ToBytes: Buf {
    fn to_bytes(mut self) -> Bytes
    where
        Self: Sized,
    {
        self.copy_to_bytes(self.remaining())
    }
}

impl<T: Buf> ToBytes for T {}
