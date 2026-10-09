//! Editing Xaero waypoints: which fields the host's dialog offers, turning the
//! confirmed values into a waypoint, and the write-back (plan W7, W10).
use super::naming::{self, MINIMAP_ROOT};
use super::overlay::{self, LAYER};
use super::waypoints::{CHOOSABLE_COLORS, DEFAULT_SET, Waypoint, WaypointFile};
use lumilio_plugin_api::map::{Dimension, EditAction, MapPoint, ObjectEdit, WorldContext};
use lumilio_plugin_api::{
    HostContext, PluginError, SettingField, SettingKind, SettingValue, Words,
};
use std::collections::BTreeMap;

/// Longest name Xaero's own dialog accepts, in characters.
pub(crate) const NAME_MAX: usize = 32;
/// Longest abbreviation shown on the map marker.
pub(crate) const INITIALS_MAX: usize = 2;
const BORDER: i64 = 30_000_000;
const FILE: &str = "mw$default_1.txt";
const HEADER: &str = "#\n#waypoint:name:initials:x:y:z:color:disabled:type:set:rotate_on_tp:tp_yaw:visibility_type:destination\n#\nsets:gui.xaero_default\n";

fn field(key: &str, zh: &str, en: &str, kind: SettingKind) -> SettingField {
    SettingField {
        key: key.into(),
        label: Words::new(zh, en),
        help: Words::default(),
        kind,
    }
}

fn number(key: &str, zh: &str, en: &str, min: i64, max: i64, default: i64) -> SettingField {
    field(key, zh, en, SettingKind::Number { min, max, default })
}

/// The dialog's fields, holding `base`'s values (or the defaults of a new
/// waypoint) as their defaults.
pub(crate) fn fields(base: Option<&Waypoint>, at: Option<MapPoint>) -> Vec<SettingField> {
    let text = |value: &str| SettingKind::Text {
        default: value.to_owned(),
    };
    let toggle = |default: bool| SettingKind::Toggle { default };
    let (x, z) = base.map_or_else(
        || at.map_or((0, 0), |at| (at.x.round() as i64, at.z.round() as i64)),
        |w| (i64::from(w.x), i64::from(w.z)),
    );
    vec![
        field("name", "名称", "Name", text(base.map_or("", |w| &w.name))),
        field(
            "initials",
            "缩写",
            "Initials",
            text(base.map_or("", |w| &w.initials)),
        ),
        number("x", "X", "X", -BORDER, BORDER, x),
        field(
            "has_y",
            "记录高度 Y",
            "Record height (Y)",
            toggle(base.is_none_or(|w| w.y.is_some())),
        ),
        number(
            "y",
            "Y",
            "Y",
            -2048,
            4096,
            base.and_then(|w| w.y).map_or(64, i64::from),
        ),
        number("z", "Z", "Z", -BORDER, BORDER, z),
        field(
            "color",
            "颜色",
            "Colour",
            SettingKind::Choice {
                options: (0..CHOOSABLE_COLORS)
                    .map(|index| index.to_string())
                    .collect(),
                default: base
                    .map(|w| w.color)
                    .filter(|color| *color < CHOOSABLE_COLORS)
                    .unwrap_or(10)
                    .to_string(),
            },
        ),
        field(
            "enabled",
            "启用",
            "Enabled",
            toggle(base.is_none_or(|w| !w.disabled)),
        ),
        field(
            "rotate",
            "传送时转向",
            "Face a direction on teleport",
            toggle(base.is_some_and(|w| w.rotate_on_tp)),
        ),
        number(
            "yaw",
            "朝向角度",
            "Facing angle",
            -999,
            9999,
            base.map_or(0, |w| i64::from(w.tp_yaw)),
        ),
    ]
}

fn invalid(id: &str) -> PluginError {
    PluginError::Unavailable(id.into())
}

fn text<'a>(values: &'a BTreeMap<String, SettingValue>, key: &str) -> Result<&'a str, PluginError> {
    match values.get(key) {
        Some(SettingValue::Text(text)) => Ok(text),
        _ => Err(PluginError::InvalidInput(format!("missing {key}"))),
    }
}

fn int(values: &BTreeMap<String, SettingValue>, key: &str) -> Result<i64, PluginError> {
    match values.get(key) {
        Some(SettingValue::Number(number)) => Ok(*number),
        _ => Err(PluginError::InvalidInput(format!("missing {key}"))),
    }
}

fn flag(values: &BTreeMap<String, SettingValue>, key: &str) -> Result<bool, PluginError> {
    match values.get(key) {
        Some(SettingValue::Toggle(flag)) => Ok(*flag),
        _ => Err(PluginError::InvalidInput(format!("missing {key}"))),
    }
}

/// A waypoint from the dialog's values. Everything the dialog does not show
/// (set, kind, visibility, destination) comes from `base`, so editing a death
/// point or a point in another set never changes what it is. Every value is
/// checked here, whatever the dialog already did.
pub(crate) fn build(
    values: &BTreeMap<String, SettingValue>,
    base: Option<&Waypoint>,
) -> Result<Waypoint, PluginError> {
    let fields = fields(base, None);
    for field in &fields {
        match values.get(&field.key) {
            Some(value) if field.kind.accepts(value) => {}
            _ => return Err(PluginError::InvalidInput(format!("bad {}", field.key))),
        }
    }
    let name = text(values, "name")?.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX || name.contains(['\n', '\r']) {
        return Err(invalid("map-edit-name-invalid"));
    }
    let initials = text(values, "initials")?.trim();
    let initials = if initials.is_empty() {
        name.chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default()
    } else {
        initials.to_owned()
    };
    if initials.chars().count() > INITIALS_MAX || initials.contains(['\n', '\r']) {
        return Err(invalid("map-edit-initials-invalid"));
    }
    let to_i32 = |key: &str| {
        i32::try_from(int(values, key)?)
            .map_err(|_| PluginError::InvalidInput(format!("bad {key}")))
    };
    let color: u8 = match values.get("color") {
        Some(SettingValue::Choice(color)) => color
            .parse()
            .map_err(|_| PluginError::InvalidInput("bad color".into()))?,
        _ => return Err(PluginError::InvalidInput("missing color".into())),
    };
    Ok(Waypoint {
        name: name.to_owned(),
        initials,
        x: to_i32("x")?,
        y: flag(values, "has_y")?.then(|| to_i32("y")).transpose()?,
        z: to_i32("z")?,
        color,
        disabled: !flag(values, "enabled")?,
        purpose: base.map_or(0, |w| w.purpose),
        set: base.map_or_else(|| DEFAULT_SET.to_owned(), |w| w.set.clone()),
        rotate_on_tp: flag(values, "rotate")?,
        tp_yaw: to_i32("yaw")?,
        visibility_type: base.map_or(0, |w| w.visibility_type),
        destination: base.is_some_and(|w| w.destination),
    })
}

/// A short fingerprint of a line, in the object id: an edit built on a line
/// that has since changed is recognised and refused.
pub(crate) fn fingerprint(raw: &str) -> String {
    let hash = raw.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{:08x}", hash & 0xffff_ffff)
}

/// The id of the waypoint on line `index` of `file`.
pub(crate) fn object_id(file: &str, index: usize, raw: &str) -> String {
    format!("{LAYER}:{file}:{index}:{}", fingerprint(raw))
}

fn parse_id(id: &str) -> Result<(&str, usize, &str), PluginError> {
    let rest = id
        .strip_prefix(LAYER)
        .and_then(|rest| rest.strip_prefix(':'))
        .ok_or_else(|| PluginError::InvalidInput("not a waypoint".into()))?;
    let mut parts = rest.rsplitn(3, ':');
    let (hash, index, file) = (parts.next(), parts.next(), parts.next());
    match (file, index.and_then(|index| index.parse().ok()), hash) {
        (Some(file), Some(index), Some(hash)) => Ok((file, index, hash)),
        _ => Err(PluginError::InvalidInput("bad waypoint id".into())),
    }
}

fn world_dir(context: &WorldContext) -> Result<&str, PluginError> {
    overlay::world_dir(context).ok_or_else(|| invalid("map-edit-unlinked"))
}

fn path_of(dir: &str, dimension: &Dimension, file: &str) -> String {
    format!(
        "{MINIMAP_ROOT}/{dir}/{}/{file}",
        naming::folder_of(dimension)
    )
}

fn parse(bytes: Vec<u8>) -> Result<WaypointFile, PluginError> {
    let text = String::from_utf8(bytes).map_err(|_| invalid("map-xaero-unreadable"))?;
    let file = WaypointFile::parse(&text);
    if file.malformed() > 0 {
        // A damaged file is shown as an error and never rewritten.
        return Err(invalid("map-xaero-unreadable"));
    }
    Ok(file)
}

fn read(
    ctx: &dyn HostContext,
    path: &str,
) -> Result<(WaypointFile, lumilio_plugin_api::FileInfo), PluginError> {
    let (bytes, info) = ctx.read_file_info(path)?;
    Ok((parse(bytes)?, info))
}

/// Where a new waypoint goes: the dimension's live waypoint file, or a new
/// default one when there is none.
fn target(ctx: &dyn HostContext, dir: &str, dimension: &Dimension) -> Result<String, PluginError> {
    let listed = ctx.list_files(&format!("{MINIMAP_ROOT}/{dir}"))?;
    let mut live: Vec<&str> = listed
        .iter()
        .filter_map(|path| {
            let (found, file) = naming::waypoint_file(path, dir)?;
            (&found == dimension).then_some(file)
        })
        .collect();
    live.sort_unstable();
    // The default multiworld's file is where the mod itself keeps new points.
    let file = live
        .iter()
        .find(|file| **file == FILE)
        .or_else(|| live.first())
        .copied()
        .unwrap_or(FILE);
    Ok(path_of(dir, dimension, file))
}

pub(crate) fn apply(ctx: &dyn HostContext, edit: &ObjectEdit) -> Result<(), PluginError> {
    if edit.overlay != LAYER {
        return Err(PluginError::InvalidInput("unknown layer".into()));
    }
    let dir = world_dir(&edit.context)?;
    let dimension = &edit.context.dimension;
    let done = match &edit.action {
        EditAction::Create { at, values } => {
            let path = target(ctx, dir, dimension)?;
            let mut values = values.clone();
            // A position placed on the map wins over the dialog's X and Z only
            // when the dialog did not carry them.
            values
                .entry("x".into())
                .or_insert(SettingValue::Number(at.x.round() as i64));
            values
                .entry("z".into())
                .or_insert(SettingValue::Number(at.z.round() as i64));
            let waypoint = build(&values, None)?;
            match ctx.read_file_info(&path) {
                Ok((bytes, info)) => {
                    let mut file = parse(bytes)?;
                    file.push(waypoint);
                    ctx.write_file(&path, file.text().as_bytes(), Some(&info))
                }
                Err(PluginError::Unavailable(_)) => {
                    let mut file = WaypointFile::parse(HEADER);
                    file.push(waypoint);
                    ctx.write_file(&path, file.text().as_bytes(), None)
                }
                Err(error) => Err(error),
            }
        }
        EditAction::Update { id, values } => {
            let (name, index, hash) = parse_id(id)?;
            let path = path_of(dir, dimension, name);
            let (mut file, info) = read(ctx, &path)?;
            let line = file
                .lines()
                .get(index)
                .ok_or_else(|| invalid("map-edit-stale"))?;
            let base = line
                .waypoint
                .clone()
                .ok_or_else(|| invalid("map-edit-stale"))?;
            if fingerprint(line.raw()) != hash {
                return Err(invalid("map-edit-stale"));
            }
            let waypoint = build(values, Some(&base))?;
            file.replace(index, waypoint)
                .ok_or_else(|| invalid("map-edit-stale"))?;
            ctx.write_file(&path, file.text().as_bytes(), Some(&info))
        }
        EditAction::Delete { id } => {
            let (name, index, hash) = parse_id(id)?;
            let path = path_of(dir, dimension, name);
            let (mut file, info) = read(ctx, &path)?;
            let line = file
                .lines()
                .get(index)
                .ok_or_else(|| invalid("map-edit-stale"))?;
            if line.waypoint.is_none() || fingerprint(line.raw()) != hash {
                return Err(invalid("map-edit-stale"));
            }
            file.remove(index);
            ctx.write_file(&path, file.text().as_bytes(), Some(&info))
        }
    };
    overlay::forget_recent();
    done
}
