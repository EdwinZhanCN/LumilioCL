//! Home presentation states, their launch transitions, and their rendering.
//!
//! Home is the fastest route from opening the launcher to playing
//! (design language §5). The hero reflects the state; Continue, Launching,
//! Playing, and Recovery put their foreground directly on the hero, and the
//! launch moment is driven by `lumilio_core::LaunchSession`.

mod buttons;
mod launching;
mod lists;
mod recovery;
mod render;

#[cfg(test)]
mod tests;

pub use self::launching::{
    BAR_SEGMENTS, filled_segments, phase_headline, phase_label, play_minutes,
};
pub use self::recovery::recovery_sentence;
pub use self::render::{ShellHomeColors, render_body, render_overlay};

use crate::hero::{HeroMode, Landmark, Scene};
use gpui::{App, Window};
use lumilio_core::{LaunchFailure, LaunchPhase, LaunchSession, LaunchSignal, LaunchStatus};
use std::rc::Rc;
use std::time::Instant;

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
