//! How the launcher reaches the network: one transport, one connection budget
//! every download shares (the launcher's own update too), the sources the
//! settings name, and documents fetched from the first source that answers.

pub(crate) use handle::Network;

pub(crate) mod fetch;
mod handle;
