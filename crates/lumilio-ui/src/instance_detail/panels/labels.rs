use lumilio_core::{ChangeKind, SessionOutcome};

pub fn change_label(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::ContentAdded => "添加了",
        ChangeKind::ContentRemoved => "移除了",
        ChangeKind::ContentEnabled => "启用了",
        ChangeKind::ContentDisabled => "停用了",
        ChangeKind::ContentUpdated => "更新了",
        ChangeKind::SettingsChanged => "修改了设置",
        ChangeKind::GameVersionChanged => "更换了游戏版本",
        ChangeKind::Repaired => "修复了",
        ChangeKind::WorldCopied => "复制了世界",
        ChangeKind::WorldDeleted => "删除了世界",
        ChangeKind::WorldImported => "导入了世界",
        ChangeKind::SnapshotCreated => "创建了快照",
        ChangeKind::SnapshotRestored => "恢复了快照",
        ChangeKind::SnapshotDeleted => "删除了快照",
    }
}

pub fn outcome_label(outcome: SessionOutcome) -> &'static str {
    match outcome {
        SessionOutcome::Clean => "正常结束",
        SessionOutcome::Crashed => "异常退出",
        SessionOutcome::FailedToStart => "没能启动",
        SessionOutcome::Stopped => "被手动停止",
        SessionOutcome::FailedToPrepare => "准备阶段失败",
        SessionOutcome::Cancelled => "启动前取消",
        SessionOutcome::Interrupted => "启动器中途退出，结果未知",
    }
}

/// What a finished export tells, in words.
pub fn export_notice(report: &lumilio_core::ExportReport, path: &std::path::Path) -> String {
    if report.lookup_failed {
        format!(
            "没能连上 Modrinth，{} 个文件都直接放进了整合包。已导出到 {}",
            report.bundled,
            path.display()
        )
    } else {
        format!(
            "已导出到 {}：{} 个文件按地址列出，{} 个放在包里",
            path.display(),
            report.linked,
            report.bundled
        )
    }
}

pub fn duration_label(seconds: u64) -> String {
    match seconds {
        0..=59 => "不到 1 分钟".to_owned(),
        60..=3599 => format!("{} 分钟", seconds / 60),
        _ => format!("{} 小时 {} 分钟", seconds / 3600, seconds % 3600 / 60),
    }
}

pub fn size_label(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    match bytes {
        0 => "—".to_owned(),
        1..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{} KB", bytes / 1024),
        _ if bytes < 1024 * MB => format!("{:.1} MB", bytes as f64 / MB as f64),
        _ => format!("{:.2} GB", bytes as f64 / (1024 * MB) as f64),
    }
}
