//! Home presentation states, their launch transitions, and their rendering.
//!
//! Home is the fastest route from opening the launcher to playing
//! (design language §5). The hero reflects the state; Continue, Launching,
//! Playing, and Recovery put their foreground directly on the hero, and the
//! launch moment is driven by `lumilio_core::LaunchSession`.

use std::rc::Rc;
use std::time::Instant;

use crate::key::{Key, KeyKind};
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, Hsla, IntoElement, SharedString,
    Window, div, ease_out_quint, prelude::*, px,
};
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use lumilio_core::{LaunchFailure, LaunchPhase, LaunchSession, LaunchSignal, LaunchStatus};

use crate::hero::{HeroMode, Landmark, Scene};
use crate::theme::motion;

/// Actions the Home canvas can ask the application layer to perform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HomeIntent {
    Import,
    Create,
    Continue,
    Recover,
    TechnicalDetails,
    CancelLaunch,
    StopGame,
}

/// A quiet item in the recent list. It carries presentation data only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentEntry {
    /// The game it opens, when it is a real one.
    pub id: Option<String>,
    pub title: String,
    pub metadata: String,
}

/// One game with something wrong, and what to do about it first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttentionRow {
    pub instance: String,
    pub name: String,
    pub title: String,
    pub detail: String,
    /// The label and meaning of the one button, if there is something to do.
    pub action: Option<(crate::instance_detail::ProblemAction, &'static str)>,
    /// Problems beyond the one shown.
    pub more: usize,
}

/// Opens a game by id.
pub type OpenHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;
/// Does a problem's action on a game.
pub type ActHandler =
    Rc<dyn Fn(String, crate::instance_detail::ProblemAction, &mut Window, &mut App)>;

/// What the page below the world can open or do.
#[derive(Clone, Default)]
pub struct HomeLinks {
    pub attention: Vec<AttentionRow>,
    pub on_open: Option<OpenHandler>,
    pub on_act: Option<ActHandler>,
}

/// Which world the hero shows for an instance. Until the domain can report
/// where the player last was, callers choose; Overworld is the default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldHint {
    #[default]
    Overworld,
    Underground,
    Redstone,
    Nether,
}

impl WorldHint {
    /// Continue's own scene, with this world as the landmark on the far hill.
    pub const fn scene(self) -> Scene {
        Scene::Hearth(match self {
            Self::Overworld => Landmark::Village,
            Self::Underground => Landmark::Mine,
            Self::Redstone => Landmark::Lamp,
            Self::Nether => Landmark::Portal,
        })
    }
}

/// The instance Home is about.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Subject {
    pub title: String,
    pub metadata: String,
    pub world: WorldHint,
}

/// Why Home is asking for attention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDetail {
    /// The previous session did not end cleanly (reported by the domain).
    Interrupted,
    /// This launch failed before the game was running.
    Failed {
        phase: LaunchPhase,
        failure: LaunchFailure,
    },
    /// The game quit with an error while it was being played.
    Crashed { code: Option<i32> },
}

/// Home can be rendered without a domain service by selecting one of these snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum HomePresentation {
    Loading,
    /// A deliberate visual-only state used before domain-backed Home data
    /// exists; the hero showcase is the whole Home canvas.
    Ambient,
    #[default]
    FirstUse,
    Continue {
        subject: Subject,
        recent: Vec<RecentEntry>,
    },
    Launching {
        subject: Subject,
        recent: Vec<RecentEntry>,
        session: LaunchSession,
    },
    Playing {
        subject: Subject,
        recent: Vec<RecentEntry>,
        since: Instant,
    },
    Recovery {
        subject: Subject,
        recent: Vec<RecentEntry>,
        detail: RecoveryDetail,
    },
}

impl HomePresentation {
    /// The only primary action for the selected state, if one exists.
    pub const fn primary_intent(&self) -> Option<HomeIntent> {
        match self {
            Self::Loading | Self::Ambient | Self::Launching { .. } | Self::Playing { .. } => None,
            Self::FirstUse => Some(HomeIntent::Import),
            Self::Continue { .. } => Some(HomeIntent::Continue),
            Self::Recovery { .. } => Some(HomeIntent::Recover),
        }
    }

    pub const fn primary_label(&self) -> Option<&'static str> {
        match self {
            Self::Loading | Self::Ambient | Self::Launching { .. } | Self::Playing { .. } => None,
            Self::FirstUse => Some("把原来的游戏带过来"),
            Self::Continue { .. } => Some("继续"),
            Self::Recovery { .. } => Some("恢复并继续"),
        }
    }

    /// Continue or Recovery → Launching with a fresh session.
    pub fn begin_launch(self) -> Self {
        match self {
            Self::Continue { subject, recent }
            | Self::Recovery {
                subject, recent, ..
            } => Self::Launching {
                subject,
                recent,
                session: LaunchSession::new(),
            },
            other => other,
        }
    }

    /// Launching → Continue, as soon as the person asks.
    pub fn cancel_launch(self) -> Self {
        match self {
            Self::Launching {
                subject, recent, ..
            } => Self::Continue { subject, recent },
            other => other,
        }
    }

    /// Advances the launch moment. Signals outside a launch or a game
    /// session are ignored.
    pub fn on_launch_signal(self, signal: LaunchSignal, now: Instant) -> Self {
        match self {
            Self::Launching {
                subject,
                recent,
                mut session,
            } => {
                session.apply(signal);
                match session.status().clone() {
                    LaunchStatus::Preparing(_) => Self::Launching {
                        subject,
                        recent,
                        session,
                    },
                    LaunchStatus::Running => Self::Playing {
                        subject,
                        recent,
                        since: now,
                    },
                    LaunchStatus::Exited { .. } | LaunchStatus::Cancelled => {
                        Self::Continue { subject, recent }
                    }
                    LaunchStatus::Failed { phase, failure } => Self::Recovery {
                        subject,
                        recent,
                        detail: RecoveryDetail::Failed { phase, failure },
                    },
                }
            }
            Self::Playing {
                subject,
                recent,
                since,
            } => match signal {
                LaunchSignal::Exited {
                    code: None | Some(0),
                } => Self::Continue { subject, recent },
                LaunchSignal::Exited { code } => Self::Recovery {
                    subject,
                    recent,
                    detail: RecoveryDetail::Crashed { code },
                },
                _ => Self::Playing {
                    subject,
                    recent,
                    since,
                },
            },
            other => other,
        }
    }

    /// What the hero shows for this state.
    pub fn hero_mode(&self) -> HeroMode {
        match self {
            Self::Loading | Self::Ambient | Self::FirstUse => HeroMode::Showcase,
            Self::Continue { subject, .. } => HeroMode::Focus(subject.world.scene()),
            Self::Launching {
                subject, session, ..
            } => HeroMode::Loading {
                scene: subject.world.scene(),
                progress: session.fraction(),
            },
            Self::Playing { subject, .. } => {
                let scene = subject.world.scene();
                HeroMode::Still {
                    scene,
                    age: scene.still_age(),
                    dim: true,
                }
            }
            // A calm night: the world waits while the problem is explained.
            Self::Recovery { .. } => HeroMode::Still {
                scene: Scene::Dawn,
                age: 0.8,
                dim: false,
            },
        }
    }

    fn recent(&self) -> &[RecentEntry] {
        match self {
            Self::Continue { recent, .. }
            | Self::Launching { recent, .. }
            | Self::Playing { recent, .. }
            | Self::Recovery { recent, .. } => recent,
            _ => &[],
        }
    }

    /// Whether this state has anything to show below the hero.
    /// Whether Home shows instance cards below the world (Continue, Launching,
    /// Playing, Recovery), as opposed to first-use copy.
    pub fn recent_is_instances(&self) -> bool {
        matches!(
            self,
            Self::Continue { .. }
                | Self::Launching { .. }
                | Self::Playing { .. }
                | Self::Recovery { .. }
        )
    }

    pub fn has_body(&self) -> bool {
        match self {
            Self::Ambient => false,
            Self::Loading | Self::FirstUse => true,
            _ => !self.recent().is_empty(),
        }
    }
}

/// The production shell starts quiet until a domain service supplies a subject.
pub const fn initial_presentation() -> HomePresentation {
    HomePresentation::Ambient
}

pub type HomeIntentHandler = Rc<dyn Fn(HomeIntent, &mut Window, &mut App)>;
/// Told when the pointer enters or leaves **继续**, so the world can answer.
pub type HoverHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Colors passed from the shell keep this module's copy and layout independent
/// from gpui-component's global theme shape.
#[derive(Clone, Copy)]
pub struct ShellHomeColors {
    pub foreground: gpui::Hsla,
    pub muted: gpui::Hsla,
    pub border: gpui::Hsla,
    pub surface: gpui::Hsla,
    pub primary: gpui::Hsla,
    pub primary_foreground: gpui::Hsla,
    pub focus: gpui::Hsla,
    pub danger: gpui::Hsla,
}

/// Page content below the world. Uses theme colours only, so it is correct
/// in light and dark appearance alike.
pub fn render_body(
    presentation: &HomePresentation,
    intent_handler: Option<HomeIntentHandler>,
    links: &HomeLinks,
    colors: ShellHomeColors,
) -> AnyElement {
    match presentation {
        HomePresentation::Loading => div()
            .text_sm()
            .text_color(colors.muted)
            .child("正在准备")
            .into_any_element(),
        HomePresentation::FirstUse => render_first_use(intent_handler, colors).into_any_element(),
        HomePresentation::Ambient => div().into_any_element(),
        other => v_flex()
            .gap_6()
            .children(render_attention(
                &links.attention,
                links.on_act.clone(),
                colors,
            ))
            .children(
                (!other.recent().is_empty())
                    .then(|| render_recent(other.recent(), links.on_open.clone(), colors)),
            )
            .into_any_element(),
    }
}

/// Foreground placed on the world for states that are about one instance.
pub fn render_overlay(
    presentation: &HomePresentation,
    intent_handler: Option<HomeIntentHandler>,
    on_continue_hover: Option<HoverHandler>,
) -> Option<AnyElement> {
    let art = ArtButtons::new();
    let (key, content): (usize, _) = match presentation {
        HomePresentation::Continue { subject, .. } => (
            0,
            render_continue(subject, intent_handler, on_continue_hover, art),
        ),
        HomePresentation::Launching {
            subject, session, ..
        } => (1, render_launching(subject, session, intent_handler, art)),
        HomePresentation::Playing { subject, since, .. } => {
            (2, render_playing(subject, *since, intent_handler, art))
        }
        HomePresentation::Recovery {
            subject, detail, ..
        } => (3, render_recovery(subject, detail, intent_handler, art)),
        _ => return None,
    };
    Some(
        v_flex()
            // Children keep their own width: nothing on the world stretches.
            .items_start()
            .child(content)
            .with_animation(
                ("home-overlay", key),
                Animation::new(motion::SCENE).with_easing(ease_out_quint()),
                |overlay, delta| overlay.opacity(delta).mt(motion::LIFT * (1. - delta)),
            )
            .into_any_element(),
    )
}

/// Foreground on world art is always light-on-dark, whatever the theme; the
/// scrim behind it guarantees the contrast.
fn on_art() -> Hsla {
    gpui::white()
}

/// Which key a button on the world is. Keys keep their colours over art
/// (design language §12): orange for the one primary, black for the rest.
#[derive(Clone, Copy)]
struct ArtStyle(KeyKind);

#[derive(Clone, Copy)]
struct ArtButtons {
    /// The orange key: the one bright thing on the art.
    primary: ArtStyle,
    /// The black key: secondary actions over the world.
    glass: ArtStyle,
}

impl ArtButtons {
    fn new() -> Self {
        Self {
            primary: ArtStyle(KeyKind::Orange),
            glass: ArtStyle(KeyKind::Black),
        }
    }
}

fn eyebrow(label: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .child(div().size(px(8.)).bg(accent))
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(on_art().opacity(0.78))
                .child(label.into()),
        )
}

fn headline(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(30.))
        .line_height(px(38.))
        .font_semibold()
        .text_color(on_art())
        .child(text.into())
}

fn caption(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(on_art().opacity(0.74))
        .child(text.into())
}

fn world_accent(world: WorldHint) -> Hsla {
    crate::hero::accent(world.scene())
}

fn render_continue(
    subject: &Subject,
    intent_handler: Option<HomeIntentHandler>,
    on_hover: Option<HoverHandler>,
    art: ArtButtons,
) -> AnyElement {
    // ia[home]: 继续 | 英雄区主按钮「继续」 | 启动当前游戏，英雄区进入启动时刻；跟随当前游戏（导航右下角芯片） | H-NAV-01
    let button = art_button(
        "home-continue",
        "继续",
        LocalActionIcon::Continue,
        HomeIntent::Continue,
        art.primary,
        intent_handler,
    )
    .large();
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow("接着上次", world_accent(subject.world)))
        .child(headline(subject.title.clone()))
        .child(caption(subject.metadata.clone()))
        .child(
            div()
                .id("home-continue-hover")
                .debug_selector(|| "home-continue".into())
                .mt(px(12.))
                .when_some(on_hover, |target, on_hover| {
                    target.on_hover(move |hovered, window, cx| on_hover(*hovered, window, cx))
                })
                .child(button),
        )
        .into_any_element()
}

/// People-facing names of the launch phases, in order.
pub const fn phase_label(phase: LaunchPhase) -> &'static str {
    match phase {
        LaunchPhase::Verifying => "检查",
        LaunchPhase::Libraries => "依赖库",
        LaunchPhase::Assets => "资源",
        LaunchPhase::Starting => "启动",
    }
}

pub const fn phase_headline(phase: LaunchPhase) -> &'static str {
    match phase {
        LaunchPhase::Verifying => "正在检查游戏文件",
        LaunchPhase::Libraries => "正在补齐依赖库",
        LaunchPhase::Assets => "正在准备资源文件",
        LaunchPhase::Starting => "正在启动游戏",
    }
}

/// Segments in the launch bar; each is a block that fills discretely.
pub const BAR_SEGMENTS: usize = 24;
const SEGMENT: f32 = 14.;
const SEGMENT_GAP: f32 = 3.;
/// Side padding inside the launch display.
const DISPLAY_PAD: f32 = 12.;
/// Room for the item counts beside the bar.
const COUNTS_WIDTH: f32 = 90.;
/// Width of the Continue button the bar grows out of.
const MORPH_FROM: f32 = 116.;

/// Filled segments for a progress fraction: never more than earned.
pub fn filled_segments(fraction: f32) -> usize {
    ((fraction.clamp(0., 1.) * BAR_SEGMENTS as f32).floor() as usize).min(BAR_SEGMENTS)
}

fn render_launching(
    subject: &Subject,
    session: &LaunchSession,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let phase = session.phase().unwrap_or(LaunchPhase::Starting);
    let filled = filled_segments(session.fraction());
    let bar_width = BAR_SEGMENTS as f32 * (SEGMENT + SEGMENT_GAP) - SEGMENT_GAP;
    let full_width = bar_width + 12. + COUNTS_WIDTH + DISPLAY_PAD * 2.;
    // Art is always dark, so its display is the night body's.
    let night = crate::theme::Body::of(true);
    let track = night.display_dim;
    let fill_color = night.display_ink;

    let segments = (0..BAR_SEGMENTS).map(|index| {
        let block = div().w(px(SEGMENT)).h(px(10.)).rounded(px(1.));
        if index < filled {
            // A block that just filled flashes white once, then takes the
            // world's colour.
            block
                .bg(fill_color)
                .with_animation(
                    ("launch-segment-filled", index),
                    Animation::new(motion::SETTLE).with_easing(ease_out_quint()),
                    move |block, delta| block.bg(gpui::white().blend(fill_color.opacity(delta))),
                )
                .into_any_element()
        } else if index == filled {
            // The block being worked on blinks in two hard steps.
            block
                .bg(track)
                .with_animation(
                    ("launch-segment-next", index),
                    Animation::new(motion::SCENE).repeat(),
                    move |block, delta| {
                        block.bg(if delta < 0.5 {
                            fill_color.opacity(0.5)
                        } else {
                            track
                        })
                    },
                )
                .into_any_element()
        } else {
            block.bg(track).into_any_element()
        }
    });

    let counts = session
        .items()
        .map(|(done, total)| format!("{done} / {total}"))
        .unwrap_or_default();

    let bar = h_flex()
        .flex_none()
        .gap(px(12.))
        .items_center()
        .px(px(DISPLAY_PAD))
        .child(h_flex().gap(px(SEGMENT_GAP)).children(segments))
        .child(
            div()
                .font_family(crate::theme::MONO_FONT)
                .text_xs()
                .text_color(night.display_label)
                .child(counts),
        );

    // The orange Continue key stretches into a black display: a clipped window
    // grows from the key's width while its face darkens to the display's.
    let morph = div()
        .h(px(40.))
        .flex()
        .items_center()
        .overflow_hidden()
        .rounded(px(6.))
        .shadow(crate::theme::display_shadow())
        .child(bar)
        .with_animation(
            "launch-morph",
            Animation::new(motion::SETTLE).with_easing(ease_out_quint()),
            move |morph, delta| {
                morph
                    .w(px(MORPH_FROM + (full_width - MORPH_FROM) * delta))
                    .bg(night.orange.blend(night.display.opacity(delta.min(1.))))
            },
        );

    let steps = LaunchPhase::ORDER.map(|step| {
        let label = div().text_xs();
        if step < phase {
            label
                .text_color(on_art().opacity(0.62))
                .child(format!("✓ {}", phase_label(step)))
        } else if step == phase {
            label
                .font_semibold()
                .text_color(on_art())
                .child(phase_label(step))
        } else {
            label
                .text_color(on_art().opacity(0.36))
                .child(phase_label(step))
        }
    });

    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow(
            format!("正在进入 · {}", subject.title),
            world_accent(subject.world),
        ))
        .child(headline(phase_headline(phase)))
        .child(div().mt(px(8.)).child(morph))
        .child(
            h_flex()
                .mt(px(4.))
                .gap(px(18.))
                .items_center()
                .child(h_flex().gap(px(14.)).children(steps))
                .child(
                    // ia[home]: 取消启动 | 启动时刻里的「取消」 | 停止本次启动，回到继续状态 | —
                    art_button(
                        "home-cancel-launch",
                        "取消",
                        LocalActionIcon::Cancel,
                        HomeIntent::CancelLaunch,
                        art.glass,
                        intent_handler,
                    )
                    .small(),
                ),
        )
        .into_any_element()
}

/// Whole minutes of play, for the calm playing readout.
pub fn play_minutes(since: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(since).as_secs() / 60
}

fn render_playing(
    subject: &Subject,
    since: Instant,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let minutes = play_minutes(since, Instant::now());
    let duration = if minutes == 0 {
        "刚刚开始".to_owned()
    } else {
        format!("已经玩了 {minutes} 分钟")
    };
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow("正在游戏中", world_accent(subject.world)))
        .child(headline(subject.title.clone()))
        .child(caption(format!("{duration} · 启动器会保持安静")))
        .child(
            div()
                .mt(px(12.))
                .debug_selector(|| "home-stop-game".into())
                // ia[home]: 结束游戏 | 游戏运行中的「结束游戏」 | 停止游戏进程 | —
                .child(art_button(
                    "home-stop-game",
                    "结束游戏",
                    LocalActionIcon::Stop,
                    HomeIntent::StopGame,
                    art.glass,
                    intent_handler,
                )),
        )
        .into_any_element()
}

/// One plain sentence about what went wrong (design language §8).
pub fn recovery_sentence(detail: &RecoveryDetail) -> String {
    let code = |code: &Option<i32>| {
        code.map(|code| format!("（退出代码 {code}）"))
            .unwrap_or_default()
    };
    match detail {
        RecoveryDetail::Interrupted => "上次没有正常结束。".to_owned(),
        RecoveryDetail::Failed {
            failure: LaunchFailure::ExitedEarly { code: exit },
            ..
        } => format!("游戏在启动时退出了{}。", code(exit)),
        RecoveryDetail::Failed { phase, .. } => {
            format!("在「{}」这一步停了下来。", phase_label(*phase))
        }
        RecoveryDetail::Crashed { code: exit } => format!("游戏意外退出了{}。", code(exit)),
    }
}

fn render_recovery(
    subject: &Subject,
    detail: &RecoveryDetail,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let title = match detail {
        RecoveryDetail::Crashed { .. } => "游戏意外退出了",
        _ => "上次没有启动成功",
    };
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow("需要看一眼", world_accent(subject.world)))
        .child(headline(title))
        .child(caption(format!(
            "{} · {}",
            subject.title,
            recovery_sentence(detail)
        )))
        .child(
            h_flex()
                .mt(px(12.))
                .gap_3()
                // ia[home]: 恢复并继续 | 启动失败后的「恢复并继续」 | 修复后重试启动 | —
                .child(art_button(
                    "home-recover",
                    "恢复并继续",
                    LocalActionIcon::Recover,
                    HomeIntent::Recover,
                    art.primary,
                    intent_handler.clone(),
                ))
                // ia[home]: 技术详情 | 启动失败后的「技术详情」 | 失败原因弹窗 | —
                .child(art_button(
                    "home-details",
                    "技术详情",
                    LocalActionIcon::TechnicalDetails,
                    HomeIntent::TechnicalDetails,
                    art.glass,
                    intent_handler,
                )),
        )
        .into_any_element()
}

fn render_first_use(
    intent_handler: Option<HomeIntentHandler>,
    colors: ShellHomeColors,
) -> impl IntoElement {
    // ia[home]: 空库：导入 | 首次使用的「把原来的游戏带过来」 | 同游戏库的导入其他启动器的游戏 | L-LIB-03
    let primary = page_button(
        "home-import",
        "把原来的游戏带过来",
        LocalActionIcon::Import,
        HomeIntent::Import,
        true,
        intent_handler.clone(),
    );
    // ia[home]: 空库：新建 | 首次使用的「新建」 | 同游戏库的新建游戏 | L-LIB-02
    let secondary = page_button(
        "home-create",
        "新建",
        LocalActionIcon::Create,
        HomeIntent::Create,
        false,
        intent_handler,
    );

    h_flex()
        .w_full()
        .flex_wrap()
        .justify_between()
        .items_center()
        .gap_6()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("第一次来到这里"),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .font_semibold()
                        .text_color(colors.foreground)
                        .child("先把熟悉的世界放在手边"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child("导入原来的游戏，其余设置之后再慢慢展开。"),
                ),
        )
        .child(h_flex().gap_3().child(primary).child(secondary))
}

/// Games with something wrong, one row each with its first remedy.
fn render_attention(
    rows: &[AttentionRow],
    on_act: Option<ActHandler>,
    colors: ShellHomeColors,
) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    // ia[home]: 需要留意：解决 | 每个有问题的游戏一行（游戏名 · 最严重的问题 · 另有几个）和一个按钮（安装/修复/更换…/去设置/去添加/查看日志…） | 点后先打开游戏页再执行 | —
    let list = rows.iter().enumerate().map(|(index, row)| {
        let button = row
            .action
            .zip(on_act.clone())
            .map(|((action, label), act)| {
                let id = row.instance.clone();
                Key::new(("home-attention-act", index))
                    .label(label)
                    .white()
                    .debug_selector(move || format!("home-attention-act-{index}"))
                    .on_click(move |_: &ClickEvent, window, cx| act(id.clone(), action, window, cx))
            });
        h_flex()
            .id(("home-attention", index))
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .py(px(10.))
            .border_b_1()
            .border_color(colors.border)
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(colors.foreground)
                            .child(format!("{} · {}", row.name, row.title)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(if row.more > 0 {
                                format!("{}（另有 {} 个问题）", row.detail, row.more)
                            } else {
                                row.detail.clone()
                            }),
                    ),
            )
            .children(button)
    });
    Some(
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(colors.muted)
                    .child("需要留意"),
            )
            .child(v_flex().w_full().children(list))
            .into_any_element(),
    )
}

fn render_recent(
    recent: &[RecentEntry],
    on_open: Option<OpenHandler>,
    colors: ShellHomeColors,
) -> impl IntoElement {
    // ia[home]: 打开最近的游戏 | 最近卡片（手型和悬停描边） | 点卡片进游戏页 | H-INSTANCE-01
    let cards = recent.iter().enumerate().map(|(index, entry)| {
        let open = entry.id.clone().zip(on_open.clone());
        v_flex()
            .id(("home-recent", index))
            .w(px(232.))
            .gap(px(4.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(6.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface)
            // A card that opens a game says so (design language §9); one that
            // does not stays still.
            .when_some(open, |card, (id, open)| {
                card.cursor_pointer()
                    .hover(move |card| card.border_color(colors.foreground))
                    .debug_selector(move || format!("home-recent-{index}"))
                    .on_click(move |_, window, cx| open(id.clone(), window, cx))
            })
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(colors.foreground)
                    .child(entry.title.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(entry.metadata.clone()),
            )
    });

    v_flex()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(colors.muted)
                .child("最近"),
        )
        .child(h_flex().flex_wrap().gap_3().children(cards))
}

/// A key on the page: orange when primary, white otherwise.
fn page_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    primary: bool,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    let button = base_button(id, label, icon, intent, intent_handler);
    if primary {
        button.primary()
    } else {
        button.white()
    }
}

/// A key on world art.
fn art_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    style: ArtStyle,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    base_button(id, label, icon, intent, intent_handler).kind(style.0)
}

/// Labelled keys carry no tooltip: the label already says it
/// (design language §9 reserves tooltips for icon-only controls).
fn base_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    let enabled = intent_handler.is_some();
    let mut button = Key::new(id)
        .label(label)
        .icon(Icon::new(icon))
        .disabled(!enabled);
    if let Some(handler) = intent_handler {
        button = button.on_click(move |_: &ClickEvent, window, cx| handler(intent, window, cx));
    }
    button
}

#[derive(Clone, Copy)]
enum LocalActionIcon {
    Import,
    Create,
    Continue,
    Recover,
    TechnicalDetails,
    Cancel,
    Stop,
}

impl gpui_component::IconNamed for LocalActionIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Import => "icons/lucide/download.svg",
            Self::Create => "icons/lucide/plus.svg",
            Self::Continue => "icons/lucide/play.svg",
            Self::Recover => "icons/lucide/refresh-cw.svg",
            Self::TechnicalDetails => "icons/lucide/info.svg",
            Self::Cancel => "icons/lucide/x.svg",
            Self::Stop => "icons/lucide/square.svg",
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lumilio_core::{LaunchFailure, LaunchPhase, LaunchSignal};

    use super::{
        BAR_SEGMENTS, HomeIntent, HomePresentation, RecentEntry, RecoveryDetail, Subject,
        WorldHint, filled_segments, play_minutes, recovery_sentence,
    };
    use crate::hero::{HeroMode, Landmark, Scene};

    fn subject() -> Subject {
        Subject {
            title: "我的世界".into(),
            metadata: "1.21 · Fabric".into(),
            world: WorldHint::Nether,
        }
    }

    fn continuing() -> HomePresentation {
        HomePresentation::Continue {
            subject: subject(),
            recent: vec![RecentEntry {
                id: None,
                title: "空岛".into(),
                metadata: "昨天".into(),
            }],
        }
    }

    #[test]
    fn each_home_state_has_at_most_one_primary_intent() {
        assert_eq!(HomePresentation::Loading.primary_intent(), None);
        assert_eq!(HomePresentation::Ambient.primary_intent(), None);
        assert_eq!(
            HomePresentation::FirstUse.primary_intent(),
            Some(HomeIntent::Import)
        );
        assert_eq!(continuing().primary_intent(), Some(HomeIntent::Continue));
        assert_eq!(continuing().begin_launch().primary_intent(), None);
    }

    #[test]
    fn production_home_starts_without_first_use_actions() {
        assert_eq!(super::initial_presentation(), HomePresentation::Ambient);
        assert!(!HomePresentation::Ambient.has_body());
        assert_eq!(HomePresentation::Ambient.hero_mode(), HeroMode::Showcase);
    }

    #[test]
    fn a_successful_launch_walks_continue_launching_playing_continue() {
        let now = Instant::now();
        assert_eq!(
            continuing().hero_mode(),
            HeroMode::Focus(Scene::Hearth(Landmark::Portal))
        );
        let mut home = continuing().begin_launch();
        assert!(matches!(home, HomePresentation::Launching { .. }));

        for signal in [
            LaunchSignal::Phase(LaunchPhase::Libraries),
            LaunchSignal::Progress { done: 3, total: 9 },
            LaunchSignal::Phase(LaunchPhase::Starting),
        ] {
            home = home.on_launch_signal(signal, now);
        }
        assert!(matches!(
            home.hero_mode(),
            HeroMode::Loading { scene: Scene::Hearth(Landmark::Portal), progress } if progress > 0.8
        ));

        home = home.on_launch_signal(LaunchSignal::Running, now);
        assert!(matches!(home, HomePresentation::Playing { since, .. } if since == now));
        assert!(matches!(
            home.hero_mode(),
            HeroMode::Still { dim: true, .. }
        ));

        home = home.on_launch_signal(LaunchSignal::Exited { code: Some(0) }, now);
        assert_eq!(home, continuing());
    }

    #[test]
    fn failures_land_in_recovery_with_a_plain_reason() {
        let now = Instant::now();
        let failed = continuing()
            .begin_launch()
            .on_launch_signal(LaunchSignal::Phase(LaunchPhase::Assets), now)
            .on_launch_signal(
                LaunchSignal::Failed(LaunchFailure::Step {
                    message: "checksum".into(),
                }),
                now,
            );
        let HomePresentation::Recovery { detail, .. } = &failed else {
            panic!("expected recovery, got {failed:?}");
        };
        assert_eq!(recovery_sentence(detail), "在「资源」这一步停了下来。");
        assert!(!recovery_sentence(detail).contains("checksum"));

        let crashed = continuing()
            .begin_launch()
            .on_launch_signal(LaunchSignal::Running, now)
            .on_launch_signal(LaunchSignal::Exited { code: Some(-1) }, now);
        assert!(matches!(
            crashed,
            HomePresentation::Recovery {
                detail: RecoveryDetail::Crashed { code: Some(-1) },
                ..
            }
        ));
        assert!(matches!(
            crashed.begin_launch(),
            HomePresentation::Launching { .. }
        ));
    }

    #[test]
    fn cancelling_returns_to_continue_and_stray_signals_are_ignored() {
        let now = Instant::now();
        assert_eq!(continuing().begin_launch().cancel_launch(), continuing());
        assert_eq!(
            continuing().on_launch_signal(LaunchSignal::Running, now),
            continuing()
        );
        assert_eq!(
            HomePresentation::FirstUse.begin_launch(),
            HomePresentation::FirstUse
        );
    }

    #[test]
    fn enabled_buttons_show_the_pointer_and_disabled_ones_do_not() {
        let handler: super::HomeIntentHandler = std::rc::Rc::new(|_, _, _| {});
        let pointer = |handler| {
            super::base_button(
                "test",
                "继续",
                super::LocalActionIcon::Continue,
                HomeIntent::Continue,
                handler,
            )
            .shows_pointer()
        };
        assert!(pointer(Some(handler)));
        assert!(!pointer(None));
    }

    #[test]
    fn the_bar_only_shows_earned_blocks() {
        assert_eq!(filled_segments(0.), 0);
        assert_eq!(filled_segments(0.999), BAR_SEGMENTS - 1);
        assert_eq!(filled_segments(1.), BAR_SEGMENTS);
        assert_eq!(filled_segments(7.), BAR_SEGMENTS);
    }

    #[test]
    fn play_time_counts_whole_minutes() {
        let start = Instant::now();
        assert_eq!(play_minutes(start, start + Duration::from_secs(59)), 0);
        assert_eq!(play_minutes(start, start + Duration::from_secs(125)), 2);
        assert_eq!(play_minutes(start + Duration::from_secs(5), start), 0);
    }
}
