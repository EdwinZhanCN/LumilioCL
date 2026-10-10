//! The theme file: what a person writes (or the launcher ships) to describe a
//! look. Built-in and local themes share this one structure. A theme names its
//! tone and any of the colours, radii and fonts below; whatever it leaves out
//! comes from the built-in body of the same tone, and [`Resolved::missing`]
//! says what was filled in.
//!
//! The shape follows Zed's theme files (a file holds several `themes`, each
//! with a `name` and an `appearance`), with colours as `#rrggbb` or
//! `#rrggbbaa`, so a colour may carry opacity.

use gpui::{Hsla, Pixels, Rgba, px};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Body;

/// Whether a theme sits on a light or a dark body. It picks the base that
/// missing keys come from and which of the two pickers lists the theme.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Light,
    Dark,
}

impl Tone {
    #[must_use]
    pub fn is_dark(self) -> bool {
        self == Self::Dark
    }

    #[must_use]
    pub fn of(dark: bool) -> Self {
        if dark { Self::Dark } else { Self::Light }
    }
}

/// A colour written as `#rrggbb` or `#rrggbbaa`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub Hsla);

impl Color {
    /// Parses `#rrggbb` or `#rrggbbaa`; the `#` is optional.
    pub fn parse(text: &str) -> Result<Self, String> {
        let digits = text.trim().trim_start_matches('#');
        let bad = || format!("`{text}` is not a colour (#rrggbb or #rrggbbaa)");
        if !digits.is_ascii() {
            return Err(bad());
        }
        let value = u32::from_str_radix(digits, 16).map_err(|_| bad())?;
        let rgba = match digits.len() {
            6 => gpui::rgb(value),
            8 => gpui::rgba(value),
            _ => return Err(bad()),
        };
        Ok(Self(rgba.into()))
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        let rgba = Rgba::from(self.0);
        let byte = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
        let (r, g, b, a) = (byte(rgba.r), byte(rgba.g), byte(rgba.b), byte(rgba.a));
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

/// Declares [`Palette`] from the colour fields of [`Body`], so a colour is
/// named once and the file, the fill-in and the built-in export cannot drift.
macro_rules! palette {
    ($($field:ident),+ $(,)?) => {
        /// The colours of a theme, one optional entry per [`Body`] colour.
        #[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
        #[serde(default)]
        pub struct Palette {
            $(#[serde(skip_serializing_if = "Option::is_none")] pub $field: Option<Color>,)+
        }

        impl Palette {
            /// Every colour of `body`, written out.
            #[must_use]
            pub fn of(body: Body) -> Self {
                Self { $($field: Some(Color(body.$field)),)+ }
            }

            /// `base` with this palette's colours on top, and the names of
            /// the colours this palette did not give.
            #[must_use]
            pub fn fill(&self, base: Body) -> (Body, Vec<&'static str>) {
                let mut body = base;
                let mut missing = Vec::new();
                $(match self.$field {
                    Some(color) => body.$field = color.0,
                    None => missing.push(stringify!($field)),
                })+
                (body, missing)
            }
        }
    };
}

palette!(
    page,
    panel,
    hairline,
    ink,
    muted,
    key_white,
    key_black,
    key_grey,
    on_white,
    on_black,
    orange,
    orange_text,
    on_orange,
    danger,
    danger_text,
    led_off,
    ok,
    display,
    display_ink,
    display_dim,
    display_label,
);

/// Type faces a theme asks for. Each is a family name; the launcher falls
/// back to its own face when the family is not installed.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Fonts {
    /// The interface's Latin and figure face.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sans: Option<String>,
    /// Values, versions, logs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mono: Option<String>,
    /// A face for Chinese text, tried before the system's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cjk: Option<String>,
}

/// One theme as written in a file.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ThemeSpec {
    pub name: String,
    pub appearance: Tone,
    /// Where the colours come from and under which licence, for themes that
    /// adapt someone else's palette.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub colors: Palette,
    /// Corner radius of controls and of dialogs, in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_lg: Option<f32>,
    #[serde(default)]
    pub fonts: Fonts,
    /// What the window itself is made of behind translucent colours. Only a
    /// theme sets it; there is no setting for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_background: Option<WindowBackground>,
}

/// The material of the window behind the page colour. A colour with opacity
/// shows this through; with `Opaque` it shows the window's plain backing.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowBackground {
    #[default]
    Opaque,
    /// Plain alpha: what is behind the window shows through.
    Transparent,
    /// What is behind the window shows through, blurred. Not every system can
    /// do this; where it cannot, the window stays as it was.
    Blurred,
}

impl WindowBackground {
    #[must_use]
    pub fn to_gpui(self) -> gpui::WindowBackgroundAppearance {
        match self {
            Self::Opaque => gpui::WindowBackgroundAppearance::Opaque,
            Self::Transparent => gpui::WindowBackgroundAppearance::Transparent,
            Self::Blurred => gpui::WindowBackgroundAppearance::Blurred,
        }
    }
}

impl ThemeSpec {
    /// A built-in body written out in full: how the launcher's own two
    /// themes travel through the same structure as any other.
    #[must_use]
    pub fn of_body(name: &str, body: Body) -> Self {
        Self {
            name: name.to_owned(),
            appearance: Tone::of(body.dark),
            source: None,
            colors: Palette::of(body),
            radius: Some(DEFAULT_RADIUS),
            radius_lg: Some(DEFAULT_RADIUS_LG),
            fonts: Fonts::default(),
            window_background: None,
        }
    }
}

pub const DEFAULT_RADIUS: f32 = 4.;
pub const DEFAULT_RADIUS_LG: f32 = 6.;
/// Larger radii turn controls into pills and break the keys' shape.
pub const MAX_RADIUS: f32 = 16.;

/// A theme with every gap filled: what the launcher actually draws with.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub name: String,
    pub tone: Tone,
    pub body: Body,
    pub radius: Pixels,
    pub radius_lg: Pixels,
    pub fonts: Fonts,
    pub window_background: WindowBackground,
    /// The colours the file did not give, filled from the built-in body.
    pub missing: Vec<&'static str>,
}

impl Resolved {
    #[must_use]
    pub fn new(spec: &ThemeSpec) -> Self {
        let (body, missing) = spec.colors.fill(Body::of(spec.appearance.is_dark()));
        let radius = |value: Option<f32>, default: f32| {
            px(value
                .filter(|value| value.is_finite())
                .unwrap_or(default)
                .clamp(0., MAX_RADIUS))
        };
        Self {
            name: spec.name.clone(),
            tone: spec.appearance,
            body,
            radius: radius(spec.radius, DEFAULT_RADIUS),
            radius_lg: radius(spec.radius_lg, DEFAULT_RADIUS_LG),
            fonts: spec.fonts.clone(),
            window_background: spec.window_background.unwrap_or_default(),
            missing,
        }
    }
}

/// Reads a theme file. The outer error is a file that is not a theme file at
/// all; each theme inside answers for itself, so one bad theme does not hide
/// the rest.
pub fn parse_file(text: &str) -> Result<Vec<Result<ThemeSpec, String>>, String> {
    #[derive(Deserialize)]
    struct File {
        themes: Vec<serde_json::Value>,
    }
    let file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(file
        .themes
        .into_iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?")
                .to_owned();
            serde_json::from_value::<ThemeSpec>(value).map_err(|error| format!("{name}: {error}"))
        })
        .collect())
}
