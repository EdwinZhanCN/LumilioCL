//! The tab's own words, Chinese and English side by side. Paths, file names
//! and schematic names are data and stay as they are.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("Litematica 投影", "Litematica schematics")
}

pub(crate) fn description() -> Words {
    Words::new(
        "在投影工作室里浏览游戏的投影文件，查看模型、统计和材料清单。",
        "Browse the game's schematic files in the schematic studio, with the model, statistics and material lists.",
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

    pub(crate) fn export_toast(&self) -> &'static str {
        self.word((
            "没有导出：这个投影读不了",
            "Not exported: this schematic cannot be read",
        ))
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
            "\u{feff}Block,Count,Stacks\r\n"
        } else {
            "\u{feff}方块,数量,组数\r\n"
        }
    }

    /// `count` items as the game stacks them, as the studio's materials say
    /// it (plan schematic-studio T25): "1 组 + 22", "3 组", "22".
    pub(crate) fn stacks(&self, count: u64) -> String {
        let (full, rest) = (count / 64, count % 64);
        let english = self.locale.starts_with("en");
        match (full, rest) {
            (0, rest) => rest.to_string(),
            (full, 0) if english => {
                format!("{full} {}", if full == 1 { "stack" } else { "stacks" })
            }
            (full, 0) => format!("{full} 组"),
            (full, rest) if english => {
                format!(
                    "{full} {} + {rest}",
                    if full == 1 { "stack" } else { "stacks" }
                )
            }
            (full, rest) => format!("{full} 组 + {rest}"),
        }
    }
}
