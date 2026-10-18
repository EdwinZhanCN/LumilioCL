//! Mojang's official mappings (`client_mappings`, ProGuard format), read at
//! bake time only (plan schematic-studio, D4): the entity dumper needs the
//! obfuscated names of a few classes, fields and methods in 1.17–1.21.x jars.
//! The mappings are neither committed nor redistributed.
//!
//! ```text
//! # comment
//! net.minecraft.client.model.geom.PartPose -> fyl:
//!     float x -> a
//!     12:14:net.minecraft.client.model.geom.PartPose offset(float,float,float) -> a
//!     20:20:void <init>(float,float):21:21 -> <init>
//! ```

use std::collections::HashMap;

use crate::release::Result;

/// One class: its obfuscated name and members by official name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ClassMapping {
    pub obfuscated: String,
    fields: HashMap<String, String>,
    /// `name(argument types)` → obfuscated name.
    methods: HashMap<String, String>,
}

impl ClassMapping {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }

    /// The method `name` taking `arguments` (official type names, comma
    /// separated, empty for none).
    pub fn method(&self, name: &str, arguments: &str) -> Option<&str> {
        self.methods
            .get(&format!("{name}({arguments})"))
            .map(String::as_str)
    }

    /// The obfuscated name of the methods called `name`, whatever they take,
    /// when they all share one (an override and its bridges do).
    pub fn method_named(&self, name: &str) -> Option<&str> {
        let prefix = format!("{name}(");
        let mut found = self
            .methods
            .iter()
            .filter(|(signature, _)| signature.starts_with(&prefix))
            .map(|(_, obfuscated)| obfuscated.as_str());
        let first = found.next()?;
        found.all(|other| other == first).then_some(first)
    }
}

/// Official class name → its mapping.
#[derive(Debug, Default)]
pub struct Mappings {
    classes: HashMap<String, ClassMapping>,
}

impl Mappings {
    pub fn parse(text: &str) -> Result<Self> {
        let mut mappings = Self::default();
        let mut current: Option<String> = None;
        for (number, line) in text.lines().enumerate() {
            let bad = || format!("mappings line {}: {line}", number + 1);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let (left, right) = line.split_once(" -> ").ok_or_else(bad)?;
            if !line.starts_with([' ', '\t']) {
                let obfuscated = right.strip_suffix(':').ok_or_else(bad)?;
                let name = left.trim().to_owned();
                mappings.classes.insert(
                    name.clone(),
                    ClassMapping {
                        obfuscated: obfuscated.to_owned(),
                        ..ClassMapping::default()
                    },
                );
                current = Some(name);
                continue;
            }
            let class = current
                .as_ref()
                .and_then(|name| mappings.classes.get_mut(name))
                .ok_or_else(bad)?;
            // `[start:end:]type name[(arguments)[:start:end]]`
            let (head, member) = left.trim().split_once(' ').ok_or_else(bad)?;
            if head.is_empty() || member.is_empty() {
                return Err(bad());
            }
            let obfuscated = right.trim().to_owned();
            match member.split_once('(') {
                Some((name, rest)) => {
                    let arguments = rest.split_once(')').ok_or_else(bad)?.0;
                    class
                        .methods
                        .insert(format!("{name}({arguments})"), obfuscated);
                }
                None => {
                    class.fields.insert(member.to_owned(), obfuscated);
                }
            }
        }
        Ok(mappings)
    }

    pub fn class(&self, name: &str) -> Option<&ClassMapping> {
        self.classes.get(name)
    }
}

const GEOM: &str = "net.minecraft.client.model.geom.";
const BUILDERS: &str = "net.minecraft.client.model.geom.builders.";

/// A class the dumper reads: its official name (after the package prefix),
/// its fields (marked when only some versions have them) and its no-argument
/// methods (marked when only some versions have them).
struct Need {
    package: &'static str,
    class: &'static str,
    /// Only some versions have the class.
    optional: bool,
    fields: &'static [(&'static str, bool)],
    methods: &'static [(&'static str, bool)],
    /// Methods the dumper finds by name and argument count, whatever they
    /// take; only where the version has them.
    named: &'static [&'static str],
}

const NEEDS: &[Need] = &[
    // Called before the layers: some read registries, which need the bootstrap.
    Need {
        package: "net.minecraft.",
        class: "SharedConstants",
        optional: true,
        fields: &[],
        methods: &[("tryDetectVersion", true)],
        named: &[],
    },
    Need {
        package: "net.minecraft.server.",
        class: "Bootstrap",
        optional: true,
        fields: &[],
        methods: &[("bootStrap", true)],
        named: &[],
    },
    Need {
        package: GEOM,
        class: "LayerDefinitions",
        optional: false,
        fields: &[],
        methods: &[("createRoots", false)],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "LayerDefinition",
        optional: false,
        fields: &[("mesh", false), ("material", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "MaterialDefinition",
        optional: false,
        fields: &[("xTexSize", false), ("yTexSize", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "MeshDefinition",
        optional: false,
        fields: &[("root", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "PartDefinition",
        optional: false,
        fields: &[("cubes", false), ("partPose", false), ("children", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "CubeDefinition",
        optional: false,
        fields: &[
            ("origin", false),
            ("dimensions", false),
            ("grow", false),
            ("mirror", false),
            ("texCoord", false),
            ("texScale", true),
            ("visibleFaces", true),
        ],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "CubeDeformation",
        optional: false,
        fields: &[("growX", false), ("growY", false), ("growZ", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: BUILDERS,
        class: "UVPair",
        optional: false,
        fields: &[("u", false), ("v", false)],
        methods: &[],
        named: &[],
    },
    Need {
        package: GEOM,
        class: "PartPose",
        optional: false,
        fields: &[
            ("x", false),
            ("y", false),
            ("z", false),
            ("xRot", false),
            ("yRot", false),
            ("zRot", false),
            ("xScale", true),
            ("yScale", true),
            ("zScale", true),
        ],
        methods: &[],
        named: &[],
    },
    // The renderers (plan schematic-studio T34, D32): every entity type's
    // renderer, built from a context holding only the model layers. Only some
    // versions have each; a version without them still gives its geometry.
    Need {
        package: "net.minecraft.client.renderer.entity.",
        class: "EntityRenderers",
        optional: true,
        fields: &[("PROVIDERS", true)],
        methods: &[],
        named: &[],
    },
    Need {
        package: "net.minecraft.client.renderer.entity.",
        class: "EntityRendererProvider",
        optional: true,
        fields: &[],
        methods: &[],
        named: &["create"],
    },
    Need {
        package: "net.minecraft.client.renderer.entity.",
        class: "EntityRendererProvider$Context",
        optional: true,
        fields: &[("modelSet", true)],
        methods: &[],
        named: &[],
    },
    Need {
        package: GEOM,
        class: "EntityModelSet",
        optional: true,
        fields: &[("roots", true)],
        methods: &[],
        named: &[],
    },
    Need {
        package: "net.minecraft.core.registries.",
        class: "BuiltInRegistries",
        optional: true,
        fields: &[("ENTITY_TYPE", true)],
        methods: &[],
        named: &[],
    },
    Need {
        package: "net.minecraft.core.",
        class: "Registry",
        optional: true,
        fields: &[("ENTITY_TYPE", true)],
        methods: &[],
        named: &["getKey"],
    },
    Need {
        package: "net.minecraft.client.renderer.entity.",
        class: "EntityRenderer",
        optional: true,
        fields: &[],
        methods: &[],
        named: &["createRenderState"],
    },
    Need {
        package: "net.minecraft.client.renderer.entity.",
        class: "LivingEntityRenderer",
        optional: true,
        fields: &[],
        methods: &[],
        named: &["getTextureLocation"],
    },
    Need {
        package: "net.minecraft.client.renderer.entity.state.",
        class: "LivingEntityRenderState",
        optional: true,
        fields: &[("isBaby", true)],
        methods: &[],
        named: &[],
    },
    // The game's own vector before 1.19.3 (JOML's afterwards, never renamed).
    Need {
        package: "com.mojang.math.",
        class: "Vector3f",
        optional: true,
        fields: &[("x", false), ("y", false), ("z", false)],
        methods: &[],
        named: &[],
    },
];

/// The names file `tools/entity-dump` reads for an obfuscated jar: official
/// name → the jar's name for every class and member it touches.
pub fn dumper_names(mappings: &Mappings) -> Result<String> {
    let mut out = String::new();
    for need in NEEDS {
        let official = format!("{}{}", need.package, need.class);
        let Some(class) = mappings.class(&official) else {
            if need.optional {
                continue;
            }
            return Err(format!("the mappings have no class {official}"));
        };
        out.push_str(&format!("class {official} {}\n", class.obfuscated));
        for (field, optional) in need.fields {
            match class.field(field) {
                Some(name) => out.push_str(&format!("field {official} {field} {name}\n")),
                None if *optional => {}
                None => return Err(format!("the mappings have no field {official}.{field}")),
            }
        }
        for (method, optional) in need.methods {
            match class.method(method, "") {
                Some(name) => out.push_str(&format!("method {official} {method} {name}\n")),
                None if *optional => {}
                None => return Err(format!("the mappings have no method {official}.{method}()")),
            }
        }
        for method in need.named {
            if let Some(name) = class.method_named(method) {
                out.push_str(&format!("method {official} {method} {name}\n"));
            }
        }
    }
    // Every class, so the renderers' and entities' class chains are written
    // in official names (`LivingEntityRenderer`, not `gfr`).
    let mut all: Vec<(&String, &ClassMapping)> = mappings.classes.iter().collect();
    all.sort_by(|a, b| a.0.cmp(b.0));
    for (official, class) in all {
        out.push_str(&format!("class {official} {}\n", class.obfuscated));
    }
    Ok(out)
}
