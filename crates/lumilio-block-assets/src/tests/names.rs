use crate::{BlockNames, ContainerError, names_in_lang_file};

fn lang(text: &str) -> std::collections::BTreeMap<String, String> {
    names_in_lang_file(text.as_bytes()).unwrap()
}

#[test]
fn a_language_file_keeps_only_the_names_of_things() {
    let names = lang(
        r#"{
            "block.minecraft.stone": "Stone",
            "item.minecraft.redstone": "Redstone Dust",
            "entity.minecraft.armor_stand": "Armor Stand",
            "block.minecraft.banner.base.white": "Fully White Field",
            "gui.done": "Done",
            "block.minecraft": "Broken"
        }"#,
    );
    let keys: Vec<&str> = names.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "block.minecraft.stone",
            "entity.minecraft.armor_stand",
            "item.minecraft.redstone"
        ]
    );
}

#[test]
fn the_newest_version_names_a_key_and_older_ids_keep_theirs() {
    let mut names = BlockNames::default();
    // Newest first, as the bake adds them.
    names.add(
        "en_us",
        lang(r#"{"block.minecraft.short_grass": "Short Grass", "block.minecraft.chain": "Iron Chain"}"#),
    );
    names.add(
        "en_us",
        lang(r#"{"block.minecraft.grass": "Grass", "block.minecraft.chain": "Chain"}"#),
    );
    assert_eq!(names.block("minecraft:chain", "en_us"), Some("Iron Chain"));
    assert_eq!(
        names.block("minecraft:short_grass", "en_us"),
        Some("Short Grass")
    );
    assert_eq!(names.block("minecraft:grass", "en_us"), Some("Grass"));
}

#[test]
fn a_block_falls_back_to_its_item_and_a_language_to_english() {
    let mut names = BlockNames::default();
    names.add(
        "en_us",
        lang(r#"{"item.minecraft.redstone": "Redstone Dust", "block.minecraft.stone": "Stone", "entity.minecraft.item_frame": "Item Frame"}"#),
    );
    names.add("zh_cn", lang(r#"{"block.minecraft.stone": "石头"}"#));
    assert_eq!(names.block("minecraft:stone[]", "zh_cn"), Some("石头"));
    assert_eq!(names.block("stone", "zh_cn"), Some("石头"));
    assert_eq!(
        names.block("minecraft:redstone", "zh_cn"),
        Some("Redstone Dust")
    );
    assert_eq!(
        names.entity("minecraft:item_frame", "zh_cn"),
        Some("Item Frame")
    );
    assert_eq!(names.block("minecraft:item_frame", "zh_cn"), None);
    assert_eq!(names.block("farmersdelight:stove", "en_us"), None);
}

#[test]
fn names_read_back_equal_and_a_damaged_file_is_an_error() {
    let mut names = BlockNames::default();
    names.add(
        "zh_cn",
        lang(r#"{"block.minecraft.smooth_stone_slab": "平滑石台阶"}"#),
    );
    let bytes = names.to_bytes().unwrap();
    assert_eq!(BlockNames::read(&bytes).unwrap(), names);

    assert_eq!(BlockNames::read(b""), Err(ContainerError::NotABundle));
    let other = zstd::encode_all(&b"LUMIBAKE\x01\x00\x00\x00{}"[..], 3).unwrap();
    assert_eq!(BlockNames::read(&other), Err(ContainerError::NotABundle));
    let newer = zstd::encode_all(&b"LUMINAME\x09\x00\x00\x00{}"[..], 3).unwrap();
    assert_eq!(
        BlockNames::read(&newer),
        Err(ContainerError::UnsupportedFormat(9))
    );
}
