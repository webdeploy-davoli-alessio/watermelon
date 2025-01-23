pub use self::authenticator::AuthenticationMethod;
pub use self::connection::{ConnectionCompression, ConnectionSecurity};
pub(crate) use self::connector::connect;
pub use self::connector::ConnectError;

mod authenticator;
mod connection;
mod connector;
