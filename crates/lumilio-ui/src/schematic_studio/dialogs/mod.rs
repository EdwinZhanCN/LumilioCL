//! The studio's operation dialogs (design language §10 dialog shape): fields
//! stacked with their labels above, 取消 and the commit key in the footer.
//! While the write runs the dialog stays open and disabled; a failure stays
//! in the dialog as one sentence under the fields; success closes it.

mod convert;
mod metadata;
mod pack_path;
mod replace;

pub(crate) use convert::{ConvertDialog, Suggested, suggestions};
#[cfg(test)]
pub(crate) use convert::{Group, Planned, detail as loss_words};
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) use form::OpenDialog;
pub(crate) use form::{DialogForm, Submission, field, note, show};
pub(crate) use metadata::MetadataDialog;
pub(crate) use pack_path::PackPathDialog;
#[cfg(test)]
pub(crate) use replace::{BlockChoice, Preview};
pub(crate) use replace::{Opening, ReplaceDialog};

mod form;
