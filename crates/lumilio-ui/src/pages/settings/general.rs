use super::super::live::LiveCtx;
use super::rows::{preference_row, row, send, with_preferences};
use super::{AFTER_LAUNCH, APPEARANCES, MOTION};
use crate::kit;
use crate::live::SettingsView;
use gpui::{AnyElement, IntoElement};
use lumilio_core::{AfterLaunch, Appearance, MotionPreference};

pub(super) fn general(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let prefs = &view.preferences;
    let appearance = match prefs.appearance {
        Appearance::System => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    };
    let after = match prefs.after_launch {
        AfterLaunch::Keep => 0,
        AfterLaunch::Hide => 1,
    };
    let motion = match prefs.motion {
        MotionPreference::System => 0,
        MotionPreference::Reduce => 1,
        MotionPreference::Full => 2,
    };
    let foreground = prefs.foreground_on_exit();
    let shown = view.clone();
    let rows = vec![
        // ia[settings]: 外观 | 通用 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存 | L-SET-01
        preference_row(
            "settings-appearance",
            "外观",
            &APPEARANCES,
            appearance,
            {
                let view = shown.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.appearance =
                            [Appearance::System, Appearance::Light, Appearance::Dark][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        // ia[settings]: 进入游戏后 | 通用 · 分段：保持 / 隐藏启动器 | 没有“关闭启动器”：启动器要守着游戏记录会话与游玩时间，关掉它游戏也会结束 | L-SET-01
        preference_row(
            "settings-after-launch",
            "进入游戏后",
            &AFTER_LAUNCH,
            after,
            {
                let view = shown.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.after_launch = [AfterLaunch::Keep, AfterLaunch::Hide][index.min(1)];
                    })
                }
            },
            ctx,
        ),
        // ia[settings]: 游戏退出后回到前台 | 通用 · 开关（默认开） | 游戏结束时把启动器带回最前面 | L-SET-01
        row(
            "settings-foreground",
            "游戏退出后回到前台",
            Some("游戏结束时把启动器带回最前面，方便接着选下一个游戏。".to_owned()),
            "",
            Some(
                kit::switch(
                    "settings-foreground-switch",
                    foreground,
                    "游戏退出后回到前台",
                    send(
                        &ctx.handler,
                        with_preferences(view, move |prefs| {
                            prefs.foreground_on_exit = Some(!foreground);
                        }),
                    ),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 减少动效 | 通用 · 分段：跟随系统 / 减少 / 完整 | 立即生效 | L-SET-01
        preference_row(
            "settings-motion",
            "减少动效",
            &MOTION,
            motion,
            {
                let view = shown;
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.motion = [
                            MotionPreference::System,
                            MotionPreference::Reduce,
                            MotionPreference::Full,
                        ][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        row(
            "settings-language",
            "语言",
            Some("目前只有简体中文。".to_owned()),
            "简体中文",
            None,
            colors,
        ),
    ];
    kit::list(rows, colors).into_any_element()
}
