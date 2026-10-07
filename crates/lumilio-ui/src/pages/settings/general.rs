use super::super::live::LiveCtx;
use super::rows::{preference_row, row, send, with_preferences};
use crate::kit;
use crate::live::SettingsView;
use crate::{tr, tr_all};
use gpui::{AnyElement, IntoElement};
use lumilio_core::{AfterLaunch, Appearance, Language, MotionPreference};

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
    let language = match prefs.language {
        Language::System => 0,
        Language::SimplifiedChinese => 1,
        Language::English => 2,
    };
    let foreground = prefs.foreground_on_exit();
    let shown = view.clone();
    let rows = vec![
        // ia[settings]: 外观 | 通用 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存
        preference_row(
            "settings-appearance",
            tr!("settings-appearance"),
            None,
            tr_all![
                "settings-follow-system",
                "settings-appearance-light",
                "settings-appearance-dark",
            ],
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
        // ia[settings]: 语言 | 通用 · 分段：跟随系统 / 简体中文 / English | 立即切换并保存，组件自带的文字一起换；跟随系统时取系统语言里第一个中文或英文，都没有则用简体中文
        preference_row(
            "settings-language",
            tr!("settings-language"),
            Some(tr!("settings-language-help")),
            tr_all![
                "settings-follow-system",
                "language-simplified-chinese",
                "language-english",
            ],
            language,
            {
                let view = shown.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.language = [
                            Language::System,
                            Language::SimplifiedChinese,
                            Language::English,
                        ][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        // ia[settings]: 进入游戏后 | 通用 · 分段：保持 / 隐藏启动器 | 没有“关闭启动器”：启动器要守着游戏记录会话与游玩时间，关掉它游戏也会结束
        preference_row(
            "settings-after-launch",
            tr!("settings-after-launch"),
            None,
            tr_all!["settings-after-launch-keep", "settings-after-launch-hide"],
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
        // ia[settings]: 游戏退出后回到前台 | 通用 · 开关（默认开） | 游戏结束时把启动器带回最前面
        row(
            "settings-foreground",
            tr!("settings-foreground"),
            Some(tr!("settings-foreground-help").to_owned()),
            "",
            Some(
                kit::switch(
                    "settings-foreground-switch",
                    foreground,
                    tr!("settings-foreground"),
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
        // ia[settings]: 减少动效 | 通用 · 分段：跟随系统 / 减少 / 完整 | 立即生效
        preference_row(
            "settings-motion",
            tr!("settings-motion"),
            None,
            tr_all![
                "settings-follow-system",
                "settings-motion-reduce",
                "settings-motion-full",
            ],
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
    ];
    kit::list(rows, colors).into_any_element()
}
