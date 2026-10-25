use lumilio_core::LaunchTuning;

use super::super::text::{
    bytes_text, commands_text, list_text, memory_help, memory_text, window_text,
};
use crate::live::SettingsView;

#[test]
fn values_read_in_plain_words() {
    assert_eq!(memory_text(None), "未设置");
    assert_eq!(memory_text(Some(4096)), "4096 MB");
    assert_eq!(list_text(&[]), "无");
    assert_eq!(list_text(&["-Da".into()]), "-Da");
    assert_eq!(
        list_text(&["-Da".into(), "-Db".into(), "-Dc".into()]),
        "-Da 等 3 项"
    );
    assert_eq!(bytes_text(0), "0 B");
    assert_eq!(bytes_text(2048), "2 KB");
    assert_eq!(bytes_text(5 * 1024 * 1024), "5 MB");
    assert_eq!(bytes_text(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
}

#[test]
fn the_window_and_commands_are_summarized() {
    let mut view = SettingsView::default();
    assert_eq!(window_text(&view), "未设置");
    assert_eq!(commands_text(&view), "无");
    view.launch = LaunchTuning {
        window_width: Some(1280),
        window_height: Some(720),
        fullscreen: Some(true),
        wrapper: Some("nice".into()),
        post_exit: Some("true".into()),
        ..LaunchTuning::default()
    };
    assert_eq!(window_text(&view), "1280 × 720 · 全屏");
    assert_eq!(commands_text(&view), "包装、退出后");
}

#[test]
fn the_memory_help_names_the_machine_when_known() {
    assert!(memory_help(Some(16_384)).contains("16.0 GB"));
    assert!(memory_help(Some(16_384)).contains("8192 MB"));
    assert!(!memory_help(None).contains("本机内存"));
}

#[test]
fn a_write_grant_names_the_files_it_may_change() {
    let grant = lumilio_plugin_api::Permission::WriteGameFiles {
        under: "schematics".into(),
        names: "*.litematic".into(),
    };
    assert_eq!(
        super::super::plugins::permission_text(&grant),
        "修改游戏的 schematics 文件夹里名为 *.litematic 的文件（先备份，游戏运行时不写）"
    );
}
