//! The 投影 tab. The host's schematic studio draws the folder (plan
//! schematic-studio, D6); the plugin keeps what is its own: when the tab
//! appears, which files it may touch, and the material list as CSV.

use lumilio_plugin_api::{ActionId, Effect, HostContext, PluginError, TabState, View};

use crate::FOLDER;
use crate::format;
use crate::text::Text;

/// Actions the studio sends for one file: `export:<path>` and `reveal:<path>`.
const EXPORT: &str = "export:";
const REVEAL: &str = "reveal:";

fn is_schematic(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".litematic")
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A `.litematic` file below `schematics/`; anything else is refused.
fn schematic_path(path: &str) -> Result<&str, PluginError> {
    let inside = path
        .strip_prefix(FOLDER)
        .is_some_and(|rest| rest.starts_with('/'));
    if inside && is_schematic(path) && !path.split('/').any(|part| part == "..") {
        Ok(path)
    } else {
        Err(PluginError::InvalidInput("not a schematic".into()))
    }
}

// ia[plugin.litematica]: 查看投影面板 | 游戏页「插件」标签里的「投影」 | 宿主画面板：左栏目录树、右侧所选文件的概要、右上「打开工作室」；3D 视口、检视器与操作在独立的工作室窗口里。插件只说明是哪个文件夹 | 布局归宿主（plan schematic-studio D6、D22）
pub(crate) fn view(_: &dyn HostContext, _: &TabState) -> Result<View, PluginError> {
    Ok(View::Studio {
        folder: FOLDER.into(),
    })
}

pub(crate) fn update(
    ctx: &dyn HostContext,
    state: TabState,
    action: &ActionId,
) -> Result<(TabState, Vec<Effect>), PluginError> {
    let text = Text::new(ctx.locale());
    let id = action.0.as_str();
    if let Some(path) = id.strip_prefix(REVEAL) {
        let path = schematic_path(path)?;
        return Ok((
            state,
            vec![Effect::RevealGameFile {
                path: path.to_owned(),
            }],
        ));
    }
    if let Some(path) = id.strip_prefix(EXPORT) {
        let path = schematic_path(path)?;
        let effects = match read_materials(ctx, path) {
            Ok(counts) => vec![Effect::SaveAs {
                suggested_name: text.export_name(file_name(path).trim_end_matches(".litematic")),
                bytes: csv(counts, &text).into_bytes(),
            }],
            Err(_) => vec![Effect::Toast(text.export_toast().into())],
        };
        return Ok((state, effects));
    }
    Ok((state, Vec::new()))
}

fn read_materials(
    ctx: &dyn HostContext,
    path: &str,
) -> Result<std::collections::BTreeMap<String, u64>, String> {
    let bytes = ctx.read_file(path).map_err(|error| error.to_string())?;
    format::parse(&bytes)?.materials()
}

fn csv(counts: std::collections::BTreeMap<String, u64>, text: &Text<'_>) -> String {
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    // A byte-order mark makes spreadsheet apps read the file as UTF-8.
    let mut out = String::from(text.csv_header());
    for (name, count) in rows {
        let name = if name.contains([',', '"', '\n']) {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name
        };
        out.push_str(&format!("{name},{count},{}\r\n", text.stacks(count)));
    }
    out
}
