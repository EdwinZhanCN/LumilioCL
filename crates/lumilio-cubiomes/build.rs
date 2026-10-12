fn main() {
    let root = "../../forks/cubiomes";
    println!("cargo:rerun-if-changed={root}");
    println!("cargo:rerun-if-changed=src/bridge.c");
    let mut build = cc::Build::new();
    build
        .include(root)
        .warnings(false)
        .flag_if_supported("-fwrapv")
        // The game's arithmetic rounds each step; fused multiply-adds would not.
        .flag_if_supported("-ffp-contract=off");
    for name in [
        "biomenoise",
        "biomes",
        "carvers",
        "decorations",
        "finders",
        "generator",
        "layers",
        "loot",
        "noise",
        "util",
        "veins",
    ] {
        build.file(format!("{root}/{name}.c"));
    }
    build.file("src/bridge.c").compile("lumilio_cubiomes");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:rustc-link-lib=m");
    }
}
