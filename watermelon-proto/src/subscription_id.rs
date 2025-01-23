use core::fmt::{self, Display};

use crate::util::{self, ParseUintError};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    pub const MIN: Self = SubscriptionId(1);
    pub const MAX: Self = SubscriptionId(u64::MAX);

    /// Converts a slice of ASCII bytes to a `SubscriptionId`.
    ///
    /// # Errors
    ///
    /// It returns an error if the bytes do not contain a valid numeric value.
    pub fn from_ascii_bytes(buf: &[u8]) -> Result<Self, ParseUintError> {
        util::parse_u64(buf).map(Self)
    }
}

impl From<u64> for SubscriptionId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<SubscriptionId> for u64 {
    fn from(value: SubscriptionId) -> Self {
        value.0
    }
}

impl Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
