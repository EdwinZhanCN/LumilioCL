//! Talking to a Yggdrasil authentication server (what authlib-injector servers
//! such as LittleSkin speak): sign in with a name and password, keep the
//! session alive, and tell the character list.
//!
//! The rules follow HMCL (`HMCLCore/.../auth/yggdrasil/YggdrasilService.java`,
//! `YggdrasilAccount.java`, Copyright (C) 2020 huangyuhui and contributors,
//! GPL-3.0-or-later). This module only speaks the
//! protocol; where secrets are kept is [`crate::accounts::credentials`].

use client::{BODY_LIMIT, snippet};
pub use client::{Profile, Session, YggdrasilClient, YggdrasilError};
#[cfg(test)]
use serde_json::{Value, json};

pub use self::server::{AuthServer, LITTLE_SKIN_URL, fetch_metadata, little_skin, locate_server};
#[cfg(test)]
use crate::accounts::account::ProfileId;
#[cfg(test)]
use crate::accounts::microsoft::Secret;
#[cfg(test)]
use crate::transfer::{HttpRequest, Transport};

mod server;

#[cfg(test)]
mod tests;

mod client;
