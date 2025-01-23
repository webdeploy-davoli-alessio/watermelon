use bytestring::ByteString;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServerError {
    #[error("subject is invalid")]
    InvalidSubject,
    #[error("permissions violation for publish")]
    PublishPermissionViolation,
    #[error("permissions violation for subscription")]
    SubscribePermissionViolation,

    #[error("unknown protocol operation")]
    UnknownProtocolOperation,

    #[error("attempted to connect to route port")]
    ConnectionAttemptedToWrongPort,

    #[error("authorization violation")]
    AuthorizationViolation,
    #[error("authorization timeout")]
    AuthorizationTimeout,
    #[error("invalid client protocol")]
    InvalidClientProtocol,
    #[error("maximum control line exceeded")]
    MaximumControlLineExceeded,
    #[error("parser error")]
    ParseError,
    #[error("secure connection, tls required")]
    TlsRequired,
    #[error("stale connection")]
    StaleConnection,
    #[error("maximum connections exceeded")]
    MaximumConnectionsExceeded,
    #[error("slow consumer")]
    SlowConsumer,
    #[error("maximum payload violation")]
    MaximumPayloadViolation,

    #[error("unknown error: {raw_message}")]
    Other { raw_message: ByteString },
}

impl ServerError {
    pub fn is_fatal(&self) -> Option<bool> {
        match self {
            Self::InvalidSubject
            | Self::PublishPermissionViolation
            | Self::SubscribePermissionViolation => Some(false),

            Self::UnknownProtocolOperation
            | Self::ConnectionAttemptedToWrongPort
            | Self::AuthorizationViolation
            | Self::AuthorizationTimeout
            | Self::InvalidClientProtocol
            | Self::MaximumControlLineExceeded
            | Self::ParseError
            | Self::TlsRequired
            | Self::StaleConnection
            | Self::MaximumConnectionsExceeded
            | Self::SlowConsumer
            | Self::MaximumPayloadViolation => Some(true),

            Self::Other { .. } => None,
        }
    }

    pub(crate) fn parse(raw_message: ByteString) -> Self {
        const PUBLISH_PERMISSIONS: &str = "Permissions Violation for Publish";
        const SUBSCRIPTION_PERMISSIONS: &str = "Permissions Violation for Subscription";

        let m = raw_message.trim();
        if m.eq_ignore_ascii_case("Invalid Subject") {
            Self::InvalidSubject
        } else if m.len() > PUBLISH_PERMISSIONS.len()
            && m[..PUBLISH_PERMISSIONS.len()].eq_ignore_ascii_case(PUBLISH_PERMISSIONS)
        {
            Self::PublishPermissionViolation
        } else if m.len() > SUBSCRIPTION_PERMISSIONS.len()
            && m[..SUBSCRIPTION_PERMISSIONS.len()].eq_ignore_ascii_case(SUBSCRIPTION_PERMISSIONS)
        {
            Self::SubscribePermissionViolation
        } else if m.eq_ignore_ascii_case("Unknown Protocol Operation") {
            Self::UnknownProtocolOperation
        } else if m.eq_ignore_ascii_case("Attempted To Connect To Route Port") {
            Self::ConnectionAttemptedToWrongPort
        } else if m.eq_ignore_ascii_case("Authorization Violation") {
            Self::AuthorizationViolation
        } else if m.eq_ignore_ascii_case("Authorization Timeout") {
            Self::AuthorizationTimeout
        } else if m.eq_ignore_ascii_case("Invalid Client Protocol") {
            Self::InvalidClientProtocol
        } else if m.eq_ignore_ascii_case("Maximum Control Line Exceeded") {
            Self::MaximumControlLineExceeded
        } else if m.eq_ignore_ascii_case("Parser Error") {
            Self::ParseError
        } else if m.eq_ignore_ascii_case("Secure Connection - TLS Required") {
            Self::TlsRequired
        } else if m.eq_ignore_ascii_case("Stale Connection") {
            Self::StaleConnection
        } else if m.eq_ignore_ascii_case("Maximum Connections Exceeded") {
            Self::MaximumConnectionsExceeded
        } else if m.eq_ignore_ascii_case("Slow Consumer") {
            Self::SlowConsumer
        } else if m.eq_ignore_ascii_case("Maximum Payload Violation") {
            Self::MaximumPayloadViolation
        } else {
            Self::Other { raw_message }
        }
    }
}
