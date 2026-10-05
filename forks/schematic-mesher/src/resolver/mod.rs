//! Block state and model resolution.
//!
//! This module handles resolving block states to concrete model variants
//! and resolving model inheritance chains.

pub mod state_resolver;
pub mod model_resolver;
pub mod multipart;

pub use state_resolver::StateResolver;
pub use model_resolver::ModelResolver;

use crate::resource_pack::{BlockModel, BlockstateDefinition, ModelVariant, ResourcePack};
use crate::types::InputBlock;
use crate::error::Result;

/// A resolved model ready for meshing.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// The fully resolved block model (with inherited elements/textures).
    pub model: BlockModel,
    /// Block-level transform (x/y rotation, uvlock).
    pub transform: crate::types::BlockTransform,
}

/// Resolve a block to its model(s).
pub fn resolve_block(
    pack: &ResourcePack,
    block: &InputBlock,
) -> Result<Vec<ResolvedModel>> {
    let state_resolver = StateResolver::new(pack);
    let model_resolver = ModelResolver::new(pack);

    // Get model variants from blockstate
    let variants = state_resolver.resolve(block)?;

    // Resolve each variant's model inheritance
    let mut resolved = Vec::new();
    for variant in variants {
        let model = model_resolver.resolve(&variant.model)?;
        resolved.push(ResolvedModel {
            model,
            transform: crate::types::BlockTransform::new(
                variant.x,
                variant.y,
                variant.uvlock,
            ),
        });
    }

    Ok(resolved)
}
