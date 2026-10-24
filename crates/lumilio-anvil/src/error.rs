use super::UnsupportedVersion;
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// The source could not give the bytes asked for.
    Source(String),
    /// The region header or a chunk header is inconsistent.
    Format(&'static str),
    /// A chunk's compression is not one the game writes.
    Compression(u8),
    /// Decompressing or parsing the chunk failed.
    Data(String),
    /// A chunk format older than the 1.13 flattening, which names blocks by
    /// number.
    Unsupported(UnsupportedVersion),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(message) | Self::Data(message) => f.write_str(message),
            Self::Format(message) => f.write_str(message),
            Self::Compression(kind) => write!(f, "unknown chunk compression {kind}"),
            Self::Unsupported(UnsupportedVersion(version)) => {
                write!(f, "chunk format {version} is not supported")
            }
        }
    }
}

impl std::error::Error for Error {}
