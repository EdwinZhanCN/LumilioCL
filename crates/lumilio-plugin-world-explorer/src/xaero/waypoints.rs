//! Reading Xaero's waypoint files without losing a byte of them.
//!
//! Line format, as the mod's `WaypointIO` writes it:
//! `waypoint:name:initials:x:y:z:color:disabled:type:set:rotate_on_tp:tp_yaw:visibility_type:destination`
//! with `:` inside the name or initials stored as `§§`, an absent Y stored as
//! `~`, and `type` 0 for an ordinary waypoint, 1 for a death point, 2 for an
//! older death point.
//!
//! Field parsing adapted from XaeroTools crates/xaero-core/src/waypoints.rs (MIT, Copyright (c) 2026 Dek), ADR 0022.
//! XaeroTools parses a file and rewrites every line, which loses comments and
//! unknown lines. This model keeps each line's original text, so a file read
//! and written back is identical byte for byte, and an edit changes only the
//! line that was edited.

pub(crate) const DEFAULT_SET: &str = "gui.xaero_default";
const COLON_ESCAPE: &str = "§§";

/// The 16 Minecraft text colours, which every Xaero version has, and then the
/// four the mod's colour enum adds (magenta, light blue, lime, pink). Whether
/// a given version uses indices 16–19 is for its sample to say (plan W17).
pub(crate) const PALETTE: [[u8; 3]; 20] = [
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0xAA],
    [0x00, 0xAA, 0x00],
    [0x00, 0xAA, 0xAA],
    [0xAA, 0x00, 0x00],
    [0xAA, 0x00, 0xAA],
    [0xFF, 0xAA, 0x00],
    [0xAA, 0xAA, 0xAA],
    [0x55, 0x55, 0x55],
    [0x55, 0x55, 0xFF],
    [0x55, 0xFF, 0x55],
    [0x55, 0xFF, 0xFF],
    [0xFF, 0x55, 0x55],
    [0xFF, 0x55, 0xFF],
    [0xFF, 0xFF, 0x55],
    [0xFF, 0xFF, 0xFF],
    [0xC7, 0x4E, 0xBD],
    [0x3A, 0xB3, 0xDA],
    [0x80, 0xC7, 0x1F],
    [0xF3, 0x8B, 0xAA],
];

/// Colours the editor offers; the others are read but not chosen (W17).
pub(crate) const CHOOSABLE_COLORS: u8 = 16;

/// The colour for an index; one the table does not know shows as grey but is
/// written back untouched.
pub(crate) fn color_rgb(index: u8) -> [u8; 3] {
    PALETTE
        .get(usize::from(index))
        .copied()
        .unwrap_or([0xAA, 0xAA, 0xAA])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Waypoint {
    pub name: String,
    pub initials: String,
    pub x: i32,
    /// `None` when the file says `~`.
    pub y: Option<i32>,
    pub z: i32,
    pub color: u8,
    pub disabled: bool,
    /// 0 ordinary, 1 death, 2 older death.
    pub purpose: i32,
    pub set: String,
    pub rotate_on_tp: bool,
    pub tp_yaw: i32,
    pub visibility_type: i32,
    pub destination: bool,
}

impl Waypoint {
    pub fn is_death(&self) -> bool {
        matches!(self.purpose, 1 | 2)
    }
}

fn unescape(field: &str) -> String {
    field.replace(COLON_ESCAPE, ":")
}

pub(crate) fn escape(field: &str) -> String {
    field.replace(':', COLON_ESCAPE)
}

fn parse_line(line: &str) -> Option<Waypoint> {
    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() < 10 || fields[0] != "waypoint" {
        return None;
    }
    let at = |index: usize| fields.get(index).copied().unwrap_or("");
    Some(Waypoint {
        name: unescape(fields[1]),
        initials: unescape(fields[2]),
        x: fields[3].parse().ok()?,
        y: if fields[4] == "~" {
            None
        } else {
            Some(fields[4].parse().ok()?)
        },
        z: fields[5].parse().ok()?,
        color: fields[6].parse().ok()?,
        disabled: fields[7] == "true",
        purpose: fields[8].parse().ok()?,
        set: fields[9].to_owned(),
        rotate_on_tp: at(10) == "true",
        tp_yaw: at(11).parse().unwrap_or(0),
        visibility_type: at(12).parse().unwrap_or(0),
        destination: at(13) == "true",
    })
}

/// The line the mod's writer produces for a waypoint, without a line ending.
pub(crate) fn format_line(waypoint: &Waypoint) -> String {
    format!(
        "waypoint:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        escape(&waypoint.name),
        escape(&waypoint.initials),
        waypoint.x,
        waypoint.y.map_or_else(|| "~".to_owned(), |y| y.to_string()),
        waypoint.z,
        waypoint.color,
        waypoint.disabled,
        waypoint.purpose,
        waypoint.set,
        waypoint.rotate_on_tp,
        waypoint.tp_yaw,
        waypoint.visibility_type,
        waypoint.destination,
    )
}

/// One line of a file exactly as it was, ending included, and what it holds if
/// it is a waypoint.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    raw: String,
    pub waypoint: Option<Waypoint>,
    /// Starts with `waypoint:` but does not parse: kept as it is, and a sign
    /// the file may be damaged or from a newer mod.
    pub malformed: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WaypointFile {
    lines: Vec<Line>,
}

impl Line {
    /// The line exactly as it was in the file, ending included.
    pub fn raw(&self) -> &str {
        &self.raw
    }
}

impl WaypointFile {
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text
                .split_inclusive('\n')
                .map(|raw| {
                    let content = raw.trim_end_matches(['\n', '\r']);
                    let is_waypoint = content.starts_with("waypoint:");
                    let waypoint = is_waypoint.then(|| parse_line(content)).flatten();
                    Line {
                        raw: raw.to_owned(),
                        malformed: is_waypoint && waypoint.is_none(),
                        waypoint,
                    }
                })
                .collect(),
        }
    }

    /// The file's text: what was read, with any edits made since.
    pub fn text(&self) -> String {
        self.lines.iter().map(|line| line.raw.as_str()).collect()
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn malformed(&self) -> usize {
        self.lines.iter().filter(|line| line.malformed).count()
    }

    /// Appends a waypoint as a new last line, in the file's own line-ending
    /// style (a bare newline for a file with no lines yet).
    pub fn push(&mut self, waypoint: Waypoint) {
        let ending = self
            .lines
            .iter()
            .rev()
            .find_map(|line| {
                let content = line.raw.trim_end_matches(['\n', '\r']);
                let ending = &line.raw[content.len()..];
                (!ending.is_empty()).then(|| ending.to_owned())
            })
            .unwrap_or_else(|| "\n".to_owned());
        // A last line that lacks its ending gets one before the new line.
        if let Some(last) = self.lines.last_mut()
            && !last.raw.ends_with('\n')
        {
            last.raw.push_str(&ending);
        }
        self.lines.push(Line {
            raw: format!("{}{ending}", format_line(&waypoint)),
            waypoint: Some(waypoint),
            malformed: false,
        });
    }

    /// Removes line `index` if it holds a waypoint; every other line stays as
    /// it was.
    pub fn remove(&mut self, index: usize) -> Option<Waypoint> {
        self.lines.get(index)?.waypoint.as_ref()?;
        self.lines.remove(index).waypoint
    }

    /// Replaces the waypoint on line `index`, keeping that line's ending style.
    /// `None` if the line is not a waypoint.
    pub fn replace(&mut self, index: usize, waypoint: Waypoint) -> Option<()> {
        let line = self.lines.get_mut(index)?;
        line.waypoint.as_ref()?;
        let ending = &line.raw[line.raw.trim_end_matches(['\n', '\r']).len()..];
        line.raw = format!("{}{ending}", format_line(&waypoint));
        line.waypoint = Some(waypoint);
        Some(())
    }
}

/// The name of a Xaero dimension in share strings.
pub(crate) fn share_dimension(dimension: &lumilio_plugin_api::map::Dimension) -> String {
    use lumilio_plugin_api::map::Dimension;
    match dimension {
        Dimension::Overworld => "Internal-overworld-waypoints".into(),
        Dimension::Nether => "Internal-the-nether-waypoints".into(),
        Dimension::End => "Internal-the-end-waypoints".into(),
        Dimension::Custom(id) => format!("Internal-{}-waypoints", id.replace(':', "-")),
    }
}

/// The string Xaero's "share" produces and its chat/clipboard import reads:
/// `xaero-waypoint:name:initials:x:y:z:color:use_yaw:yaw:dimension`.
pub(crate) fn share_string(
    waypoint: &Waypoint,
    dimension: &lumilio_plugin_api::map::Dimension,
) -> String {
    format!(
        "xaero-waypoint:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        escape(&waypoint.name),
        escape(&waypoint.initials),
        waypoint.x,
        waypoint.y.map_or_else(|| "~".to_owned(), |y| y.to_string()),
        waypoint.z,
        waypoint.color,
        waypoint.rotate_on_tp,
        waypoint.tp_yaw,
        share_dimension(dimension),
    )
}
