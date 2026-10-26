//! The function an annotation is in or above, so a generated row names a
//! symbol that code search can find, not only a file.

use std::ops::RangeInclusive;

use syn::spanned::Spanned;
use syn::visit::Visit;

/// The named functions of one source file and the lines each spans, 1-based
/// and including its attributes and documentation.
pub(super) struct Functions(Vec<(String, RangeInclusive<usize>)>);

impl Functions {
    /// Every function in `source`; none when it does not parse, so its rows
    /// name only the file.
    pub(super) fn parse(source: &str) -> Self {
        let mut collect = Collect {
            owners: Vec::new(),
            found: Vec::new(),
        };
        if let Ok(file) = syn::parse_file(source) {
            collect.visit_file(&file);
        }
        Self(collect.found)
    }

    /// The function that starts right below the annotation on 1-based `line`
    /// (only comments in between), otherwise the innermost one around it.
    /// `lines` is the same source, indexed from 0.
    pub(super) fn at(&self, line: usize, lines: &[&str]) -> Option<&str> {
        let code = lines
            .iter()
            .enumerate()
            .skip(line)
            .find(|(_, text)| {
                let text = text.trim();
                !text.is_empty() && !text.starts_with("//")
            })
            .map_or(usize::MAX, |(index, _)| index + 1);
        let below = self
            .0
            .iter()
            .find(|(_, span)| (line + 1..=code).contains(span.start()));
        let around = || {
            self.0
                .iter()
                .filter(|(_, span)| span.contains(&line))
                .min_by_key(|(_, span)| span.end() - span.start())
        };
        below.or_else(around).map(|(name, _)| name.as_str())
    }
}

struct Collect {
    /// The type of each `impl` or trait being visited, innermost last;
    /// `None` when it is not a plain path.
    owners: Vec<Option<String>>,
    found: Vec<(String, RangeInclusive<usize>)>,
}

impl Collect {
    fn add(&mut self, name: String, span: proc_macro2::Span) {
        self.found.push((name, span.start().line..=span.end().line));
    }

    /// `Type::method` inside an `impl` or trait, else the bare name.
    fn method(&self, name: &syn::Ident) -> String {
        match self.owners.last() {
            Some(Some(owner)) => format!("{owner}::{name}"),
            _ => name.to_string(),
        }
    }
}

impl<'ast> Visit<'ast> for Collect {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.add(item.sig.ident.to_string(), item.span());
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let owner = match &*item.self_ty {
            syn::Type::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        };
        self.owners.push(owner);
        syn::visit::visit_item_impl(self, item);
        self.owners.pop();
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.add(self.method(&item.sig.ident), item.span());
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.owners.push(Some(item.ident.to_string()));
        syn::visit::visit_item_trait(self, item);
        self.owners.pop();
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.add(self.method(&item.sig.ident), item.span());
        syn::visit::visit_trait_item_fn(self, item);
    }
}
