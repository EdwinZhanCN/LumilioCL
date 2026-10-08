use std::time::{Duration, Instant};

use lumilio_plugin_api::{
    ActionId, Effect, HostContext, ImageData, KeyKind, ListItem, PluginError, TabState, Tone, View,
};

use crate::FOLDER;
use crate::format::{self, Litematic, Metadata};
use crate::text::Text;

/// Files read for one list; more are shown by name only.
const READ_BUDGET: Duration = Duration::from_secs(3);
const MAX_ITEMS: usize = 300;
const OPEN: &str = "open:";

fn is_schematic(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".litematic")
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The file's name without its extension: what people know a schematic by,
/// in the game's menus and in their own folders.
fn stem(path: &str) -> &str {
    let name = file_name(path);
    name.get(..name.len().saturating_sub(".litematic".len()))
        .filter(|_| is_schematic(name))
        .unwrap_or(name)
}

/// Folders between `schematics/` and the file, if any.
fn folder(path: &str) -> Option<String> {
    let inner = path.strip_prefix(FOLDER)?.trim_start_matches('/');
    inner.rsplit_once('/').map(|(dir, _)| dir.to_owned())
}

fn open_path(state: &TabState) -> Option<&str> {
    state.get("open").and_then(serde_json::Value::as_str)
}

pub(crate) fn view(ctx: &dyn HostContext, state: &TabState) -> Result<View, PluginError> {
    match open_path(state) {
        Some(path) => detail(ctx, path),
        None => list(ctx),
    }
}

fn list(ctx: &dyn HostContext) -> Result<View, PluginError> {
    let text = Text::new(ctx.locale());
    let files: Vec<String> = ctx
        .list_files(FOLDER)?
        .into_iter()
        .filter(|path| is_schematic(path))
        .collect();
    if files.is_empty() {
        return Ok(View::Empty {
            title: text.list_empty_title().into(),
            message: text.list_empty_message().into(),
        });
    }
    let started = Instant::now();
    let mut items = Vec::new();
    for path in files.iter().take(MAX_ITEMS) {
        let mut item = ListItem {
            id: path.clone(),
            title: stem(path).to_owned(),
            tags: folder(path).into_iter().collect(),
            ..ListItem::default()
        };
        if started.elapsed() > READ_BUDGET {
            item.subtitle = Some(text.not_read().into());
        } else {
            match read(ctx, path) {
                Ok(file) => {
                    let meta = &file.metadata;
                    item.subtitle = Some(summary(meta, path));
                    item.value = meta.total_blocks.map(|total| text.blocks(total));
                    item.image = meta.preview.as_ref().map(|(side, rgba)| ImageData {
                        width: *side,
                        height: *side,
                        rgba: rgba.clone(),
                    });
                    // ia[plugin.litematica]: 打开一份投影 | 列表行（缩略图、名称、作者与尺寸、方块数） | 进入详情：事实、材料清单；读不了的文件只在自己那一行写「读不了」，不影响列表
                    item.open = Some(ActionId::new(format!("{OPEN}{path}")));
                }
                Err(_) => item.subtitle = Some(text.unreadable().into()),
            }
        }
        items.push(item);
    }
    Ok(View::List { items })
}

fn summary(meta: &Metadata, path: &str) -> String {
    let mut parts = Vec::new();
    if let Some(author) = &meta.author {
        parts.push(author.clone());
    }
    if let Some((x, y, z)) = meta.size {
        parts.push(format!("{x}×{y}×{z}"));
    }
    if parts.is_empty() {
        parts.push(file_name(path).to_owned());
    }
    parts.join(" · ")
}

fn read(ctx: &dyn HostContext, path: &str) -> Result<Litematic, String> {
    let bytes = ctx.read_file(path).map_err(|error| error.to_string())?;
    format::parse(&bytes)
}

fn detail(ctx: &dyn HostContext, path: &str) -> Result<View, PluginError> {
    let text = Text::new(ctx.locale());
    // ia[plugin.litematica]: 返回列表 | 详情页次要键 | 回到投影列表
    let back = View::Key {
        id: ActionId::new("back"),
        label: text.back().into(),
        kind: KeyKind::Ghost,
        destructive: false,
    };
    let file = match read(ctx, path) {
        Ok(file) => file,
        Err(_) => {
            return Ok(View::Section {
                title: stem(path).to_owned(),
                children: vec![
                    View::Empty {
                        title: text.unreadable().into(),
                        message: text.detail_read_message().into(),
                    },
                    back,
                ],
            });
        }
    };
    let meta = &file.metadata;
    let mut facts = Vec::new();
    let mut fact = |label: &str, value: Option<String>| {
        if let Some(value) = value {
            facts.push((label.to_owned(), value));
        }
    };
    fact(text.schematic_name(), meta.name.clone());
    fact(text.author(), meta.author.clone());
    fact(
        text.size(),
        meta.size.map(|(x, y, z)| format!("{x} × {y} × {z}")),
    );
    fact(
        text.total_blocks(),
        meta.total_blocks.map(|n| n.to_string()),
    );
    fact(text.regions(), meta.region_count.map(|n| n.to_string()));
    fact(text.modified(), meta.modified_ms.map(date));
    fact(text.file(), Some(path.to_owned()));
    let mut children = Vec::new();
    if let Some(description) = &meta.description {
        children.push(View::Text {
            text: description.clone(),
            tone: Tone::Secondary,
        });
    }
    // ia[plugin.litematica]: 3D 预览 | 详情页的「3D 预览」卡片和键 | 打开单独的预览窗口，用游戏自己的贴图画出整个投影，可旋转缩放；游戏没装时没有贴图；Linux 上没有这个键 | 宿主负责窗口，插件只在视图里写「这里有个模型」
    children.push(View::Model {
        file: path.to_owned(),
    });
    match file.materials() {
        Ok(counts) => {
            let mut rows: Vec<(String, u64)> = counts.into_iter().collect();
            rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            children.push(View::Section {
                title: text.materials().into(),
                children: vec![if rows.is_empty() {
                    View::Empty {
                        title: text.no_blocks().into(),
                        message: text.only_air().into(),
                    }
                } else {
                    View::Table {
                        columns: vec![text.block_column().into(), text.count_column().into()],
                        rows: rows
                            .into_iter()
                            .map(|(name, count)| vec![name, count.to_string()])
                            .collect(),
                    }
                }],
            });
        }
        Err(_) => children.push(View::Empty {
            title: text.materials_unreadable().into(),
            message: text.materials_unreadable_message().into(),
        }),
    }
    children.extend([
        // ia[plugin.litematica]: 在文件夹中显示 | 详情页次要键 | 在访达或资源管理器里选中这份投影文件
        View::Key {
            id: ActionId::new("reveal"),
            label: text.show_in_folder().into(),
            kind: KeyKind::Ghost,
            destructive: false,
        },
        // ia[plugin.litematica]: 导出材料清单（CSV） | 详情页主要键 | 弹出保存对话框，选了位置才写入；按数量从多到少，表头为「方块,数量」 | 文件带 UTF-8 标记，表格软件直接读中文
        View::Key {
            id: ActionId::new("export"),
            label: text.export_csv().into(),
            kind: KeyKind::Primary,
            destructive: false,
        },
        back,
    ]);
    Ok(View::Detail {
        title: stem(path).to_owned(),
        subtitle: folder(path),
        image: meta.preview.as_ref().map(|(side, rgba)| ImageData {
            width: *side,
            height: *side,
            rgba: rgba.clone(),
        }),
        facts,
        children,
    })
}

pub(crate) fn update(
    ctx: &dyn HostContext,
    state: TabState,
    action: &ActionId,
) -> Result<(TabState, Vec<Effect>), PluginError> {
    let text = Text::new(ctx.locale());
    let id = action.0.as_str();
    if let Some(path) = id.strip_prefix(OPEN) {
        if !is_schematic(path) {
            return Err(PluginError::InvalidInput("not a schematic".into()));
        }
        return Ok((serde_json::json!({ "open": path }), Vec::new()));
    }
    match (id, open_path(&state)) {
        ("back", _) => Ok((TabState::Null, Vec::new())),
        ("reveal", Some(path)) => Ok((
            state.clone(),
            vec![Effect::RevealGameFile {
                path: path.to_owned(),
            }],
        )),
        ("export", Some(path)) => {
            let effects = match read(ctx, path).and_then(|file| file.materials()) {
                Ok(counts) => vec![Effect::SaveAs {
                    suggested_name: text
                        .export_name(file_name(path).trim_end_matches(".litematic")),
                    bytes: csv(counts, &text).into_bytes(),
                }],
                Err(_) => vec![Effect::Toast(text.export_toast().into())],
            };
            Ok((state, effects))
        }
        _ => Ok((state, Vec::new())),
    }
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
        out.push_str(&format!("{name},{count}\r\n"));
    }
    out
}

/// `YYYY-MM-DD HH:MM` in UTC.
fn date(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60
    )
}

#[cfg(test)]
pub(crate) fn date_for_tests(ms: i64) -> String {
    date(ms)
}
