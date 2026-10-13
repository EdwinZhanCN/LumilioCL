//! CurseForge plugin words, Chinese and English side by side. CurseForge
//! itself is a proper noun and stays as it is.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("CurseForge", "CurseForge")
}

pub(crate) fn description(has_key: bool) -> Words {
    if has_key {
        Words::new(
            "查找、安装和更新 CurseForge 上的模组、整合包、资源包与光影。",
            "Find, install and update mods, modpacks, resource packs and shaders from CurseForge.",
        )
    } else {
        Words::new(
            "查找、安装和更新 CurseForge 上的内容。这个版本的启动器没有内置 CurseForge API 密钥，需要密钥才能使用。",
            "Find, install and update content from CurseForge. This build of the launcher has no CurseForge API key, which it needs.",
        )
    }
}

/// A category's name in the launcher's language. CurseForge names its
/// categories in English only; unknown ones keep CurseForge's name.
pub(crate) fn category(id: u64, english: &str, locale: &str) -> String {
    if locale.starts_with("en") {
        return english.to_owned();
    }
    chinese_category(id).map_or_else(|| english.to_owned(), str::to_owned)
}

fn chinese_category(id: u64) -> Option<&'static str> {
    Some(match id {
        // Mods
        421 => "前置与库",
        426 => "附属",
        422 => "冒险与角色扮演",
        434 => "盔甲、工具与武器",
        6821 => "漏洞修复",
        424 => "装饰",
        9026 => "创造模式",
        5299 => "教育",
        436 => "食物",
        10775 => "恐怖",
        4906 => "MCreator",
        419 => "魔法",
        423 => "地图与信息",
        425 => "杂项",
        6814 => "性能",
        4558 => "红石",
        435 => "服务器工具",
        420 => "存储",
        412 => "科技",
        4671 => "Twitch 联动",
        5191 => "实用与便利",
        406 => "世界生成",
        407 => "生物群系",
        410 => "维度",
        411 => "生物",
        408 => "矿物与资源",
        409 => "结构",
        4843 => "自动化",
        417 => "能源",
        415 => "能源、流体与物品运输",
        416 => "农业",
        418 => "遗传",
        414 => "玩家传送",
        413 => "加工",
        6145 => "空岛",
        // Modpacks
        4475 => "冒险与角色扮演",
        4483 => "战斗与 PvP",
        9243 => "专家",
        4476 => "探索",
        4482 => "超大型",
        4487 => "FTB 官方整合包",
        4479 => "硬核",
        7418 => "恐怖",
        4473 => "魔法",
        4480 => "地图",
        4477 => "小游戏",
        4484 => "多人",
        4478 => "任务",
        4474 => "科幻",
        4736 => "空岛",
        4481 => "小型轻量",
        4472 => "科技",
        5128 => "原版增强",
        // Resource packs
        398 => "512x 及更高",
        404 => "动态",
        5193 => "数据包",
        5244 => "字体包",
        402 => "中世纪",
        405 => "杂项",
        4465 => "模组支持",
        401 => "现代",
        400 => "照片写实",
        399 => "蒸汽朋克",
        403 => "传统",
        // Shaders
        6554 => "奇幻",
        6553 => "写实",
        6555 => "原版",
        _ => return None,
    })
}
