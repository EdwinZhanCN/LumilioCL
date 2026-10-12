//! Reading a release-notes file: a summary paragraph, then `##` sections of
//! `-` items (the fixed sections of `assets/icons/PACKAGING.md` §5). Items
//! keep their inline Markdown; the dialog renders each one.

/// A release's notes, split for the dialog.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Notes {
    pub summary: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Section {
    /// Empty for items written before the first heading.
    pub title: String,
    pub items: Vec<String>,
}

pub(super) fn parse(text: &str) -> Notes {
    let text = without_comments(text);
    let mut notes = Notes::default();
    // A line that continues the last item or paragraph, until a blank line.
    let mut open = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            open = false;
        } else if let Some(title) = heading(trimmed) {
            notes.sections.push(Section {
                title: title.to_owned(),
                items: Vec::new(),
            });
            open = false;
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            section(&mut notes).items.push(item.trim().to_owned());
            open = true;
        } else if open {
            let last = if notes.sections.is_empty() {
                &mut notes.summary
            } else {
                let items = &mut section(&mut notes).items;
                items.last_mut().expect("an open item exists")
            };
            join(last, trimmed);
        } else if notes.sections.is_empty() {
            join(&mut notes.summary, trimmed);
            open = true;
        } else {
            // A paragraph inside a section reads as one more item.
            section(&mut notes).items.push(trimmed.to_owned());
            open = true;
        }
    }
    notes.sections.retain(|section| !section.items.is_empty());
    notes
}

fn heading(line: &str) -> Option<&str> {
    let title = line.trim_start_matches('#');
    (title.len() < line.len() && title.starts_with(' ')).then(|| title.trim())
}

fn section(notes: &mut Notes) -> &mut Section {
    if notes.sections.is_empty() {
        notes.sections.push(Section::default());
    }
    notes.sections.last_mut().expect("just ensured")
}

/// Joins a wrapped line: Chinese runs on without a space, Latin text takes one.
fn join(text: &mut String, line: &str) {
    let ascii_seam = text.chars().last().is_some_and(|c| c.is_ascii())
        || line.chars().next().is_some_and(|c| c.is_ascii());
    if !text.is_empty() && ascii_seam {
        text.push(' ');
    }
    text.push_str(line);
}

fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start..]
            .find("-->")
            .map_or("", |end| &rest[start + end + 3..]);
    }
    out.push_str(rest);
    out
}
