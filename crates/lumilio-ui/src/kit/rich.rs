//! Long text a content source wrote: project bodies and changelogs, in
//! Markdown or HTML.

use gpui::ElementId;
use gpui_component::text::TextView;
use lumilio_core::TextFormat;

/// A text view for `text` in its source's format.
pub fn rich_text(id: impl Into<ElementId>, text: String, format: TextFormat) -> TextView {
    match format {
        TextFormat::Markdown => TextView::markdown(id, text),
        TextFormat::Html => TextView::html(id, paragraph_breaks(&text)),
    }
}

/// The HTML renderer breaks lines only between blocks, so a `<br>` inside a
/// paragraph (CurseForge's descriptions are full of them) would run the
/// lines together; it becomes a paragraph break instead.
pub(crate) fn paragraph_breaks(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tag = &rest[at..];
        let end = tag.find('>').map(|end| end + 1);
        let is_break = end.is_some_and(|end| {
            let inner = tag[1..end - 1].trim().trim_end_matches('/').trim();
            inner.eq_ignore_ascii_case("br")
        });
        match (is_break, end) {
            (true, Some(end)) => {
                out.push_str("</p><p>");
                rest = &tag[end..];
            }
            _ => {
                out.push('<');
                rest = &tag[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
