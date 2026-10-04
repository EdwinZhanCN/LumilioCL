use crate::fetch::FetchError;
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum DiscoverError {
    Decode(String),
    Fetch(FetchError),
}

impl Display for DiscoverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(f, "unexpected Modrinth answer: {message}"),
            Self::Fetch(error) => write!(f, "Modrinth unavailable: {error}"),
        }
    }
}

impl Error for DiscoverError {}

impl From<FetchError> for DiscoverError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

pub(super) fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, DiscoverError> {
    serde_json::from_slice(bytes).map_err(|error| DiscoverError::Decode(error.to_string()))
}
