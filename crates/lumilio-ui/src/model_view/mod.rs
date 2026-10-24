//! Schematic viewer: in a modal (`ModelView`) or inline in the schematic
//! studio's viewport. Meshing and GPU readback stay on the worker; a frame
//! clock runs only while first-person input is captured.

mod input;
mod modal;
mod slice;

#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use gpui::prelude::*;
#[cfg(test)]
use gpui::{Context, MouseButton, ScrollDelta, Window, div, px};
#[cfg(test)]
use input::{FlightInput, Mode};
#[cfg(test)]
use lumilio_core::{AssetNotice, ModelPreview};
#[cfg(test)]
use lumilio_schematic_render::{Angle, SceneError, View};
pub use modal::ModelView;
pub(crate) use slice::Cells;
#[cfg(test)]
pub(crate) use viewer::SLICING_CLEARANCE;
#[cfg(test)]
use viewer::physical_size;
use viewer::{CLICK_SLOP, Load, MARK_WIDTH, State, undrawable_entities_note, undrawable_note};
pub(crate) use viewer::{
    Inspected, TOOL_STRIP_CLEARANCE, Viewer, angle_labels, base_label, file_label,
};
#[cfg(test)]
use worker::{Event, Job, Request};
#[cfg(test)]
pub(crate) use worker::{Job as WorkerJob, Work};

#[cfg(test)]
use crate::key::KeySize;
#[cfg(test)]
use crate::theme;

#[cfg(test)]
mod tests;
mod worker;

mod render;

mod interaction;

mod viewer;
