use super::TranslationError;
use html5ever::{
    LocalName, QualName, namespace_url, ns, parse_fragment, serialize, tendril::TendrilSink,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};

const BATCH_CHARS: usize = 4_000;

pub(super) struct Document {
    dom: RcDom,
    nodes: Vec<Handle>,
    parts: Vec<Vec<String>>,
}

fn html(text: &str) -> Result<String, TranslationError> {
    let options = markdown::Options {
        compile: markdown::CompileOptions {
            allow_dangerous_html: true,
            ..Default::default()
        },
        parse: markdown::ParseOptions::gfm(),
    };
    markdown::to_html_with_options(text, &options).map_err(|_| TranslationError::Format)
}

impl Document {
    pub(super) fn parse(original: &str) -> Result<Self, TranslationError> {
        let dom = parse_fragment(
            RcDom::default(),
            Default::default(),
            QualName::new(None, ns!(html), LocalName::from("div")),
            Vec::new(),
        )
        .one(html(original)?);
        let mut nodes = Vec::new();
        collect(&dom.document, false, &mut nodes);
        let parts = nodes
            .iter()
            .map(|node| {
                let NodeData::Text { contents } = &node.data else {
                    unreachable!()
                };
                split(&contents.borrow())
            })
            .collect();
        Ok(Self { dom, nodes, parts })
    }

    pub(super) fn texts(&self) -> Vec<String> {
        self.parts.iter().flatten().cloned().collect()
    }

    pub(super) fn replace(&mut self, translated: &[String]) -> Result<String, TranslationError> {
        if translated.len() != self.parts.iter().map(Vec::len).sum::<usize>() {
            return Err(TranslationError::Protocol);
        }
        let mut index = 0;
        for (node, parts) in self.nodes.iter().zip(&self.parts) {
            let NodeData::Text { contents } = &node.data else {
                unreachable!()
            };
            let replacement = translated[index..index + parts.len()].concat();
            *contents.borrow_mut() = replacement.into();
            index += parts.len();
        }
        let mut bytes = Vec::new();
        serialize(
            &mut bytes,
            &SerializableHandle::from(self.dom.document.clone()),
            Default::default(),
        )
        .map_err(|_| TranslationError::Format)?;
        String::from_utf8(bytes).map_err(|_| TranslationError::Format)
    }
}

fn collect(node: &Handle, protected: bool, texts: &mut Vec<Handle>) {
    let protected = protected
        || matches!(&node.data, NodeData::Element { name, .. } if matches!(name.local.as_ref(), "code" | "pre" | "script" | "style" | "textarea"));
    if !protected
        && let NodeData::Text { contents } = &node.data
        && !contents.borrow().trim().is_empty()
    {
        texts.push(node.clone());
    }
    for child in node.children.borrow().iter() {
        collect(child, protected, texts);
    }
}

fn split(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut length = 0;
    for ch in text.chars() {
        let count = ch.len_utf16();
        if length + count > BATCH_CHARS {
            parts.push(std::mem::take(&mut part));
            length = 0;
        }
        part.push(ch);
        length += count;
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

pub(super) fn batches(texts: &[String]) -> Vec<&[String]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut length = 0;
    for (index, text) in texts.iter().enumerate() {
        let count = text.encode_utf16().count();
        if index > start && (length + count > BATCH_CHARS || index - start >= 50) {
            batches.push(&texts[start..index]);
            start = index;
            length = 0;
        }
        length += count;
    }
    if start < texts.len() {
        batches.push(&texts[start..]);
    }
    batches
}

pub(super) fn safe_summary(original: &str, translated: &str) -> bool {
    let skeleton = |text: &str| {
        let mut doc = Document::parse(text).ok()?;
        let blanks = vec![String::new(); doc.texts().len()];
        doc.replace(&blanks).ok()
    };
    match (skeleton(original), skeleton(translated)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
