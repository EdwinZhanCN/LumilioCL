//! The tab's own words, Chinese and English side by side. Paths, file names
//! and schematic names are data and stay as they are.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("Litematica 投影", "Litematica schematics")
}

pub(crate) fn description() -> Words {
    Words::new(
        "浏览游戏里的投影文件，查看尺寸和材料清单。",
        "Browse the game's schematic files, with sizes and material lists.",
    )
}

pub(crate) fn tab_title() -> Words {
    Words::new("投影", "Schematics")
}

fn pick((zh_cn, en): (&'static str, &'static str), locale: &str) -> &'static str {
    if locale.starts_with("en") && !en.is_empty() {
        en
    } else {
        zh_cn
    }
}

/// The words of the Litematica tab for one language.
pub(crate) struct Text<'a> {
    locale: &'a str,
}

impl<'a> Text<'a> {
    pub(crate) fn new(locale: &'a str) -> Self {
        Self { locale }
    }

    fn word(&self, pair: (&'static str, &'static str)) -> &'static str {
        pick(pair, self.locale)
    }

    pub(crate) fn list_empty_title(&self) -> &'static str {
        self.word(("还没有投影", "No schematics yet"))
    }

    pub(crate) fn list_empty_message(&self) -> &'static str {
        self.word((
            "把 .litematic 文件放进游戏的 schematics 文件夹，或在游戏里用 Litematica 保存投影。",
            "Put .litematic files in the game's schematics folder, or save a schematic with Litematica in the game.",
        ))
    }

    pub(crate) fn not_read(&self) -> &'static str {
        self.word(("还没有读取", "Not read yet"))
    }

    pub(crate) fn unreadable(&self) -> &'static str {
        self.word(("读不了", "Cannot read"))
    }

    pub(crate) fn detail_read_message(&self) -> &'static str {
        self.word((
            "这个文件损坏了，或者太大。",
            "This file is damaged or too large.",
        ))
    }

    pub(crate) fn back(&self) -> &'static str {
        self.word(("返回列表", "Back to list"))
    }

    pub(crate) fn schematic_name(&self) -> &'static str {
        self.word(("投影名称", "Schematic name"))
    }

    pub(crate) fn author(&self) -> &'static str {
        self.word(("作者", "Author"))
    }

    pub(crate) fn size(&self) -> &'static str {
        self.word(("尺寸", "Size"))
    }

    pub(crate) fn total_blocks(&self) -> &'static str {
        self.word(("方块总数", "Total blocks"))
    }

    pub(crate) fn regions(&self) -> &'static str {
        self.word(("区域", "Regions"))
    }

    pub(crate) fn modified(&self) -> &'static str {
        self.word(("修改时间", "Modified"))
    }

    pub(crate) fn file(&self) -> &'static str {
        self.word(("文件", "File"))
    }

    pub(crate) fn materials(&self) -> &'static str {
        self.word(("材料清单", "Materials"))
    }

    pub(crate) fn no_blocks(&self) -> &'static str {
        self.word(("没有方块", "No blocks"))
    }

    pub(crate) fn only_air(&self) -> &'static str {
        self.word(("这个投影里只有空气。", "This schematic contains only air."))
    }

    pub(crate) fn block_column(&self) -> &'static str {
        self.word(("方块", "Block"))
    }

    pub(crate) fn count_column(&self) -> &'static str {
        self.word(("数量", "Count"))
    }

    pub(crate) fn materials_unreadable(&self) -> &'static str {
        self.word(("材料清单读不了", "Materials cannot be read"))
    }

    pub(crate) fn materials_unreadable_message(&self) -> &'static str {
        self.word((
            "这个投影的方块数据损坏了，或者太大。",
            "This schematic's block data is damaged or too large.",
        ))
    }

    pub(crate) fn show_in_folder(&self) -> &'static str {
        self.word(("在文件夹中显示", "Show in folder"))
    }

    pub(crate) fn export_csv(&self) -> &'static str {
        self.word(("导出材料清单（CSV）", "Export materials (CSV)"))
    }

    pub(crate) fn export_toast(&self) -> &'static str {
        self.word((
            "没有导出：这个投影读不了",
            "Not exported: this schematic cannot be read",
        ))
    }

    /// `2 blocks` / `2 个方块`, with an English singular.
    pub(crate) fn blocks(&self, count: i64) -> String {
        if self.locale.starts_with("en") {
            if count == 1 {
                "1 block".to_owned()
            } else {
                format!("{count} blocks")
            }
        } else {
            format!("{count} 个方块")
        }
    }

    pub(crate) fn export_name(&self, stem: &str) -> String {
        if self.locale.starts_with("en") {
            format!("{stem}-materials.csv")
        } else {
            format!("{stem}-材料清单.csv")
        }
    }

    pub(crate) fn csv_header(&self) -> &'static str {
        if self.locale.starts_with("en") {
            "\u{feff}Block,Count\r\n"
        } else {
            "\u{feff}方块,数量\r\n"
        }
    }
}
