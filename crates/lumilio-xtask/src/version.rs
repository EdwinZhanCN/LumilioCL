//! The release version. `[workspace.package] version` is its only source; every
//! platform field is derived here (assets/icons/PACKAGING.md §0).

use std::fmt;

/// A SemVer 2.0.0 version: `MAJOR.MINOR.PATCH[-PRE][+BUILD]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
    pub build: Option<String>,
}

impl Version {
    /// The workspace version this tool was built with.
    pub fn workspace() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("the workspace version is SemVer")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let invalid = |why: &str| format!("{text:?} is not a SemVer version: {why}");
        let (rest, build) = match text.split_once('+') {
            Some((rest, build)) => (rest, Some(identifiers(build, false).map_err(invalid)?)),
            None => (text, None),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((core, pre)) => (core, Some(identifiers(pre, true).map_err(invalid)?)),
            None => (rest, None),
        };
        let numbers: Vec<&str> = core.split('.').collect();
        let [major, minor, patch] = numbers[..] else {
            return Err(invalid("expected MAJOR.MINOR.PATCH"));
        };
        Ok(Self {
            major: number(major).map_err(invalid)?,
            minor: number(minor).map_err(invalid)?,
            patch: number(patch).map_err(invalid)?,
            pre,
            build,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }

    /// `MAJOR.MINOR.PATCH`, for fields that only take numbers: the macOS
    /// `CFBundleShortVersionString` and `CFBundleVersion`.
    pub fn numeric(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// The four-part number Windows VERSIONINFO and Inno `VersionInfoVersion`
    /// want.
    pub fn windows(&self) -> String {
        format!("{}.0", self.numeric())
    }

    /// The Debian package version. `~` sorts before the release it precedes,
    /// so `0.2.0~beta.1` upgrades cleanly to `0.2.0`, as SemVer orders them.
    pub fn debian(&self) -> String {
        let mut text = self.numeric();
        if let Some(pre) = &self.pre {
            text.push('~');
            text.push_str(pre);
        }
        if let Some(build) = &self.build {
            text.push('+');
            text.push_str(build);
        }
        text
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.numeric())?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        if let Some(build) = &self.build {
            write!(f, "+{build}")?;
        }
        Ok(())
    }
}

fn number(text: &str) -> Result<u64, &'static str> {
    if text.len() > 1 && text.starts_with('0') {
        return Err("numbers have no leading zeros");
    }
    text.parse()
        .map_err(|_| "MAJOR, MINOR and PATCH are numbers")
}

fn identifiers(text: &str, numeric_rules: bool) -> Result<String, &'static str> {
    for part in text.split('.') {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err("identifiers are non-empty [0-9A-Za-z-]");
        }
        if numeric_rules
            && part.len() > 1
            && part.starts_with('0')
            && part.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("numeric pre-release identifiers have no leading zeros");
        }
    }
    Ok(text.to_owned())
}
