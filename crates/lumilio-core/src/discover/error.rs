use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq)]
pub enum DiscoverError {
    /// No content source plugin is enabled, or the one that was has stopped.
    NoSource,
    /// The source plugin is on but this call did not complete.
    Unavailable(String),
    /// The plugin's answer cannot be used with this launcher.
    Invalid(String),
}

impl Display for DiscoverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSource => f.write_str("没有可用的内容源"),
            Self::Unavailable(message) => write!(f, "内容源不可用：{message}"),
            Self::Invalid(message) => write!(f, "内容源的回答无法使用：{message}"),
        }
    }
}

impl Error for DiscoverError {}
