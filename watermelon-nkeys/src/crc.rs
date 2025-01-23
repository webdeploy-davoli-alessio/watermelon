#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Crc16(u16);

impl Crc16 {
    pub(crate) fn compute(buf: &[u8]) -> Self {
        Self(crc::Crc::<u16>::new(&crc::CRC_16_XMODEM).checksum(buf))
    }

    pub(crate) fn from_raw_encoded(val: [u8; 2]) -> Self {
        Self::from_raw(u16::from_le_bytes(val))
    }

    pub(crate) fn from_raw(val: u16) -> Self {
        Self(val)
    }

    #[expect(dead_code)]
    pub(crate) fn to_raw(&self) -> u16 {
        self.0
    }

    pub(crate) fn to_raw_encoded(&self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}
