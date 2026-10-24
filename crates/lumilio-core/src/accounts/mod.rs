//! The people who play: offline profiles, Microsoft and third-party
//! (authlib-injector) sign-ins, where their secrets live, and the
//! [`Accounts`] capability that keeps their sessions valid.

pub(crate) use sign_in::Accounts;
pub use third_party::ThirdPartySignIn;

pub(crate) mod account;
pub(crate) mod credentials;
pub(crate) mod injector;
pub(crate) mod microsoft;
mod sign_in;
mod third_party;
pub(crate) mod yggdrasil;
