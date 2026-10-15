//! The block states one game version knows, read from its baked blockstates
//! (plan schematic-studio, T14): what a backward conversion checks a
//! schematic against.
//!
//! A blockstate names only the properties its models depend on. A property it
//! does not mention (`waterlogged` on stairs, say) may still exist in that
//! version, so the catalog answers "not mentioned", never "does not exist".

use std::collections::{BTreeMap, BTreeSet};

use crate::{BakedAssets, BakedBlockstate, Condition};

/// The values a blockstate gives one property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Values {
    /// Variant keys list every value the version knows.
    Listed(BTreeSet<String>),
    /// Multipart conditions name the property but need not list every value.
    Open,
}

impl Values {
    /// Whether `value` is known, as far as the blockstate tells.
    #[must_use]
    pub fn allows(&self, value: &str) -> bool {
        match self {
            Self::Listed(values) => values.contains(value),
            Self::Open => true,
        }
    }
}

/// Block id → the properties its blockstate mentions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockCatalog {
    blocks: BTreeMap<String, BTreeMap<String, Values>>,
}

impl BlockCatalog {
    /// The catalog of one baked pack's blockstates.
    #[must_use]
    pub fn of(assets: &BakedAssets) -> Self {
        Self::from_blockstates(&assets.blockstates)
    }

    #[must_use]
    pub fn from_blockstates(blockstates: &BTreeMap<String, BakedBlockstate>) -> Self {
        let mut catalog = Self::default();
        for (id, blockstate) in blockstates {
            catalog.add_block(id);
            match blockstate {
                BakedBlockstate::Variants(variants) => {
                    for key in variants.keys() {
                        catalog.add_variant(id, key);
                    }
                }
                BakedBlockstate::Multipart(cases) => {
                    for condition in cases.iter().filter_map(|case| case.when.as_ref()) {
                        catalog.add_condition(id, condition);
                    }
                }
            }
        }
        catalog
    }

    /// Records `id` as a block of this version.
    pub fn add_block(&mut self, id: &str) {
        self.blocks.entry(id.to_owned()).or_default();
    }

    /// Records a variant key such as `axis=x,waterlogged=false` (`` for a
    /// block without properties).
    pub fn add_variant(&mut self, id: &str, key: &str) {
        let properties = self.blocks.entry(id.to_owned()).or_default();
        for pair in key.split(',').filter(|pair| !pair.is_empty()) {
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            match properties
                .entry(name.to_owned())
                .or_insert_with(|| Values::Listed(BTreeSet::new()))
            {
                Values::Listed(values) => {
                    values.insert(value.to_owned());
                }
                Values::Open => {}
            }
        }
    }

    /// Records a property named by a multipart condition.
    pub fn add_open_property(&mut self, id: &str, name: &str) {
        self.blocks
            .entry(id.to_owned())
            .or_default()
            .insert(name.to_owned(), Values::Open);
    }

    fn add_condition(&mut self, id: &str, condition: &Condition) {
        match condition {
            Condition::Any(all) | Condition::All(all) => {
                for condition in all {
                    self.add_condition(id, condition);
                }
            }
            Condition::Properties(properties) => {
                for name in properties.keys() {
                    self.add_open_property(id, name);
                }
            }
        }
    }

    /// Every block id, in order (T24: the targets a replacement offers).
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.blocks.keys().map(String::as_str)
    }

    #[must_use]
    pub fn has_block(&self, id: &str) -> bool {
        self.blocks.contains_key(id)
    }

    /// What the blockstate of `id` says about `property`; `None` when it does
    /// not mention it (or there is no such block).
    #[must_use]
    pub fn property(&self, id: &str, property: &str) -> Option<&Values> {
        self.blocks.get(id)?.get(property)
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
