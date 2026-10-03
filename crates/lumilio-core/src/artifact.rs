use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A Maven-style package identity used by Minecraft release metadata.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageCoordinate {
    namespace: String,
    component: String,
    release: String,
    variant: Option<String>,
    extension: String,
}

impl PackageCoordinate {
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[must_use]
    pub fn component(&self) -> &str {
        &self.component
    }

    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    #[must_use]
    pub fn extension(&self) -> &str {
        &self.extension
    }

    #[must_use]
    pub fn with_variant(&self, variant: Option<String>) -> Self {
        Self {
            namespace: self.namespace.clone(),
            component: self.component.clone(),
            release: self.release.clone(),
            variant,
            extension: self.extension.clone(),
        }
    }

    #[must_use]
    pub fn file_name(&self) -> String {
        let variant = self
            .variant
            .as_ref()
            .map(|value| format!("-{value}"))
            .unwrap_or_default();
        format!(
            "{}-{}{}.{}",
            self.component, self.release, variant, self.extension
        )
    }

    #[must_use]
    pub fn repository_path(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.namespace.replace('.', "/"),
            self.component,
            self.release,
            self.file_name()
        )
    }
}

impl FromStr for PackageCoordinate {
    type Err = CoordinateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut extension_parts = value.split('@');
        let identity = extension_parts.next().unwrap_or_default();
        let explicit_extension = extension_parts.next();
        if extension_parts.next().is_some() {
            return Err(CoordinateError::new(value));
        }

        let extension = explicit_extension.unwrap_or("jar");
        if extension.is_empty() {
            return Err(CoordinateError::new(value));
        }

        let segments = identity.split(':').collect::<Vec<_>>();
        if !(segments.len() == 3 || segments.len() == 4)
            || segments.iter().any(|segment| segment.is_empty())
        {
            return Err(CoordinateError::new(value));
        }

        Ok(Self {
            namespace: segments[0].to_owned(),
            component: segments[1].to_owned(),
            release: segments[2].to_owned(),
            variant: segments.get(3).map(|segment| (*segment).to_owned()),
            extension: extension.to_owned(),
        })
    }
}

impl Display for PackageCoordinate {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.namespace, self.component, self.release
        )?;
        if let Some(variant) = &self.variant {
            write!(formatter, ":{variant}")?;
        }
        if self.extension != "jar" {
            write!(formatter, "@{}", self.extension)?;
        }
        Ok(())
    }
}

impl Serialize for PackageCoordinate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PackageCoordinate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateError {
    value: String,
}

impl CoordinateError {
    fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}

impl Display for CoordinateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid package coordinate: {}", self.value)
    }
}

impl Error for CoordinateError {}

#[cfg(test)]
mod tests {
    use super::PackageCoordinate;
    use std::str::FromStr;

    #[test]
    fn default_extension_is_not_rendered() {
        let coordinate = PackageCoordinate::from_str("net.example:client:1.0").unwrap();

        assert_eq!(coordinate.extension(), "jar");
        assert_eq!(coordinate.to_string(), "net.example:client:1.0");
    }
}
