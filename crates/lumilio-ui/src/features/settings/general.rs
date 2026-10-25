use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::Sizable as _;
use gpui_component::select::Select;
use lumilio_core::{AfterLaunch, Language, MotionPreference};

use super::page::SettingsCtx;
use super::rows::{preference_row, row, send, with_preferences};
use crate::live::SettingsView;
use crate::{kit, tr, tr_all};

pub(super) fn general(view: &SettingsView, ctx: &SettingsCtx) -> AnyElement {
    let colors = ctx.colors;
    let prefs = &view.preferences;
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
        // ia[settings]: 开启翻译 | 通用 · 开关（默认关） | 开启后发现列表与详情页头自动显示中文简介（先 MCIM，再 launcher.lumilio.org），正文点击翻译；关闭立即恢复原文
        row(
            "settings-translation",
            tr!("settings-translation"),
            Some(tr!("settings-translation-help").to_owned()),
            "",
            Some(
                kit::switch(
                    "settings-translation-switch",
                    prefs.translation.enabled,
                    tr!("settings-translation"),
                    send(
                        &ctx.handler,
                        with_preferences(view, {
                            let enabled = prefs.translation.enabled;
                            move |prefs| prefs.translation.enabled = !enabled
                        }),
                    ),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 翻译提供商 | 通用 · 下拉框 | 正文用的实时翻译；目前只有微软 Edge 并默认选中，MCIM 只供简介、不在此列
        row(
            "settings-translation-provider",
            tr!("settings-translation-provider"),
            None,
            "",
            Some(
                div()
                    .w(px(180.))
                    .child(
                        Select::new(&ctx.controls.translation_provider)
                            .small()
                            .disabled(!prefs.translation.enabled),
                    )
                    .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 清除翻译缓存 | 通用 · 按钮 | 删除本地保存的译文，下次翻译重新请求；结果以提示告知
        row(
            "settings-translation-cache",
            tr!("settings-translation-cache"),
            Some(tr!("settings-translation-cache-help").to_owned()),
            "",
            Some(
                kit::ghost(
                    "settings-translation-clear",
                    tr!("translation-clear"),
                    send(&ctx.handler, crate::live::LiveIntent::ClearTranslationCache),
                )
                .into_any_element(),
            ),
            colors,
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
