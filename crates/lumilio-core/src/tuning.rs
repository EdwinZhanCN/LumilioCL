//! How the game is started, beyond which game it is: window, arguments,
//! environment and commands. These are the launcher-wide defaults; an
//! instance can override some of them (Instance.Settings).

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// Largest window side the launcher accepts, in pixels.
pub const MAX_WINDOW_SIDE: u32 = 16_384;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct LaunchTuning {
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub jvm_arguments: Vec<String>,
    pub game_arguments: Vec<String>,
    pub environment: Vec<EnvVar>,
    /// Runs before the game starts; a failure stops the launch.
    pub pre_launch: Option<String>,
    /// Prefixes the Java command (for example `gamemoderun` or `mangohud`).
    pub wrapper: Option<String>,
    /// Runs after the game exits; a failure is only reported.
    pub post_exit: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TuningError {
    /// A window needs both sides, each 1..=[`MAX_WINDOW_SIDE`].
    WindowSize,
    /// Names are non-empty and free of `=`, spaces and control characters.
    EnvironmentName(String),
    /// An argument or command contains a character that cannot be passed on.
    ControlCharacter,
    /// The wrapper command has an unclosed quote or no program.
    Wrapper,
    /// The quick-play world or server is not usable.
    QuickPlay,
}

impl Display for TuningError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowSize => write!(
                f,
                "window size needs both width and height, each 1 to {MAX_WINDOW_SIDE}"
            ),
            Self::EnvironmentName(name) => {
                write!(f, "environment variable name {name:?} is not valid")
            }
            Self::ControlCharacter => {
                f.write_str("arguments and commands cannot contain control characters")
            }
            Self::Wrapper => f.write_str("the wrapper command is not a valid command line"),
            Self::QuickPlay => f.write_str("the quick-play target is not usable"),
        }
    }
}

impl Error for TuningError {}

fn plain(text: &str) -> bool {
    !text
        .chars()
        .any(|ch| ch == '\0' || (ch.is_control() && ch != '\t'))
}

impl LaunchTuning {
    /// Checks every field; a rejected value changes nothing.
    pub fn validate(&self) -> Result<(), TuningError> {
        match (self.window_width, self.window_height) {
            (None, None) => {}
            (Some(width), Some(height))
                if (1..=MAX_WINDOW_SIDE).contains(&width)
                    && (1..=MAX_WINDOW_SIDE).contains(&height) => {}
            _ => return Err(TuningError::WindowSize),
        }
        for argument in self.jvm_arguments.iter().chain(&self.game_arguments) {
            if !plain(argument) {
                return Err(TuningError::ControlCharacter);
            }
        }
        for variable in &self.environment {
            let name = &variable.name;
            if name.is_empty()
                || name.contains('=')
                || name.chars().any(|ch| ch.is_whitespace() || ch.is_control())
            {
                return Err(TuningError::EnvironmentName(name.clone()));
            }
            if !plain(&variable.value) {
                return Err(TuningError::ControlCharacter);
            }
        }
        for command in [&self.pre_launch, &self.post_exit].into_iter().flatten() {
            if !plain(command) {
                return Err(TuningError::ControlCharacter);
            }
        }
        if let Some(wrapper) = &self.wrapper
            && split_words(wrapper).is_none_or(|words| words.is_empty())
        {
            return Err(TuningError::Wrapper);
        }
        Ok(())
    }

    /// The tuning an instance actually gets: its own JVM arguments replace the
    /// defaults' when it has any, and each override in `own` replaces the
    /// matching default; everything else comes from the defaults.
    #[must_use]
    pub fn for_instance(&self, instance_jvm_arguments: &[String], own: &InstanceLaunch) -> Self {
        let mut effective = own.apply(self.clone());
        if !instance_jvm_arguments.is_empty() {
            effective.jvm_arguments = instance_jvm_arguments.to_vec();
        }
        effective
    }

    /// Trims commands and drops blank ones, so "nothing" has one spelling.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let clean = |value: Option<String>| {
            value
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        self.pre_launch = clean(self.pre_launch);
        self.wrapper = clean(self.wrapper);
        self.post_exit = clean(self.post_exit);
        self.jvm_arguments = clean_lines(self.jvm_arguments);
        self.game_arguments = clean_lines(self.game_arguments);
        self
    }
}

fn clean_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Splits a command line into words the way a shell would for plain cases:
/// whitespace separates words, single or double quotes group them, and a
/// backslash escapes the next character outside single quotes. `None` for an
/// unclosed quote.
#[must_use]
pub fn split_words(text: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(open), ch) if ch == open => quote = None,
            (Some('\''), ch) => current.push(ch),
            (_, '\\') => current.push(chars.next()?),
            (Some(_), ch) => current.push(ch),
            (None, '\'' | '"') => {
                quote = Some(ch);
                started = true;
            }
            (None, ch) if ch.is_whitespace() => {
                if started || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, ch) => current.push(ch),
        }
    }
    if quote.is_some() {
        return None;
    }
    if started || !current.is_empty() {
        words.push(current);
    }
    Some(words)
}

/// Where to go when the game starts, instead of the main menu.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickPlay {
    /// A saved world, by its folder name under `saves/`.
    World(String),
    /// A multiplayer server, `host` or `host:port`.
    Server(String),
}

/// What an instance sets for itself on top of the launcher defaults. `None`
/// follows the default; clearing a field removes the override instead of
/// copying the current default into it. An empty command or list means
/// "none on purpose", which is different from following the default.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct InstanceLaunch {
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub game_arguments: Option<Vec<String>>,
    pub environment: Option<Vec<EnvVar>>,
    pub pre_launch: Option<String>,
    pub wrapper: Option<String>,
    pub post_exit: Option<String>,
    pub after_launch: Option<AfterLaunch>,
    pub quick_play: Option<QuickPlay>,
}

/// Longest world folder name or server address accepted.
const QUICK_PLAY_LIMIT: usize = 255;

/// Why a quick-play target cannot be used.
#[must_use]
pub fn quick_play_problem(target: &QuickPlay) -> Option<&'static str> {
    match target {
        QuickPlay::World(name) => {
            if name.is_empty() || name.len() > QUICK_PLAY_LIMIT {
                Some("世界名不能为空")
            } else if name.contains(['/', '\\']) || name == "." || name == ".." || !plain(name) {
                Some("世界名不是一个有效的存档文件夹")
            } else {
                None
            }
        }
        QuickPlay::Server(address) => {
            let address = address.trim();
            let (host, port) = match address.rsplit_once(':') {
                Some((host, port)) if !host.contains(':') => (host, Some(port)),
                _ => (address, None),
            };
            if host.is_empty()
                || host.len() > QUICK_PLAY_LIMIT
                || host
                    .chars()
                    .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '/')
            {
                Some("服务器地址不是有效的主机名")
            } else if port.is_some_and(|port| port.parse::<u16>().map_or(true, |port| port == 0)) {
                Some("端口需要是 1 到 65535 的数字")
            } else {
                None
            }
        }
    }
}

/// True when the game version is known to be too old to start directly in a
/// world (before 1.20). Anything not recognizably old counts as possible; the
/// release's own rules have the last word when the game starts.
#[must_use]
pub fn quick_play_world_unsupported(game_version: &str) -> bool {
    let mut parts = game_version.split('.');
    match (parts.next(), parts.next()) {
        (Some("1"), Some(minor)) => minor
            .split(|ch: char| !ch.is_ascii_digit())
            .next()
            .and_then(|minor| minor.parse::<u32>().ok())
            .is_some_and(|minor| minor < 20),
        _ => false,
    }
}

impl InstanceLaunch {
    /// Checks the overrides the way [`LaunchTuning::validate`] checks defaults.
    pub fn validate(&self) -> Result<(), TuningError> {
        let mut as_tuning = LaunchTuning {
            window_width: self.window_width,
            window_height: self.window_height,
            fullscreen: self.fullscreen,
            game_arguments: self.game_arguments.clone().unwrap_or_default(),
            environment: self.environment.clone().unwrap_or_default(),
            pre_launch: self.pre_launch.clone(),
            wrapper: self.wrapper.clone(),
            post_exit: self.post_exit.clone(),
            ..LaunchTuning::default()
        };
        // An explicitly empty wrapper is "none on purpose", not a bad command.
        if as_tuning
            .wrapper
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            as_tuning.wrapper = None;
        }
        as_tuning.validate()?;
        if self
            .quick_play
            .as_ref()
            .and_then(quick_play_problem)
            .is_some()
        {
            return Err(TuningError::QuickPlay);
        }
        Ok(())
    }

    /// Trims commands (keeping an empty one as "none on purpose") and drops
    /// blank lines.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let trim = |value: Option<String>| value.map(|text| text.trim().to_owned());
        self.pre_launch = trim(self.pre_launch);
        self.wrapper = trim(self.wrapper);
        self.post_exit = trim(self.post_exit);
        self.game_arguments = self.game_arguments.map(clean_lines);
        self.quick_play = self.quick_play.map(|target| match target {
            QuickPlay::World(name) => QuickPlay::World(name.trim().to_owned()),
            QuickPlay::Server(address) => QuickPlay::Server(address.trim().to_owned()),
        });
        self
    }

    /// True when nothing is overridden.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// The defaults with these overrides on top.
    #[must_use]
    pub fn apply(&self, mut base: LaunchTuning) -> LaunchTuning {
        if self.window_width.is_some() || self.window_height.is_some() {
            base.window_width = self.window_width;
            base.window_height = self.window_height;
        }
        if let Some(fullscreen) = self.fullscreen {
            base.fullscreen = Some(fullscreen);
        }
        if let Some(arguments) = &self.game_arguments {
            base.game_arguments.clone_from(arguments);
        }
        if let Some(environment) = &self.environment {
            base.environment.clone_from(environment);
        }
        let command = |own: &Option<String>, inherited: Option<String>| match own {
            Some(text) if text.is_empty() => None,
            Some(text) => Some(text.clone()),
            None => inherited,
        };
        base.pre_launch = command(&self.pre_launch, base.pre_launch.take());
        base.wrapper = command(&self.wrapper, base.wrapper.take());
        base.post_exit = command(&self.post_exit, base.post_exit.take());
        base
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

/// What the launcher window does once the game is running. There is no
/// "close": the launcher supervises the game to record its session and play
/// time, so quitting would end the game with it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterLaunch {
    #[default]
    Keep,
    Hide,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreference {
    /// Follow the operating system.
    #[default]
    System,
    Reduce,
    Full,
}

/// Launcher-wide preferences that are about the launcher, not the game.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub appearance: Appearance,
    pub after_launch: AfterLaunch,
    /// `None` means the default, which is to come back to the front.
    pub foreground_on_exit: Option<bool>,
    pub motion: MotionPreference,
    /// How the Library is ordered (0 recent, 1 name, 2 created) and which
    /// loader it shows (0 all; 1 vanilla, 2 Fabric, 3 Forge, 4 NeoForge,
    /// 5 Quilt). The launcher only remembers them.
    pub library_sort: u8,
    pub library_loader: u8,
    pub discover: DiscoverPreferences,
}

/// What Discover remembers between visits (Modrinth App keeps the same four
/// things: its advanced exclusions, whether that group is open, the
/// photosensitivity warning and "hide already installed" on modpacks).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct DiscoverPreferences {
    /// Advanced exclusions the person keeps switched on, by option id.
    pub advanced_exclusions: Vec<String>,
    pub advanced_open: bool,
    /// The photosensitivity warning was dismissed for good.
    pub photosensitivity_warning_dismissed: bool,
    pub hide_installed_modpacks: bool,
}

impl Preferences {
    #[must_use]
    pub fn foreground_on_exit(&self) -> bool {
        self.foreground_on_exit.unwrap_or(true)
    }
}

/// Fewest and most files downloaded at once.
pub const DOWNLOAD_CONCURRENCY: std::ops::RangeInclusive<u32> = 1..=32;

#[cfg(test)]
mod tests;
