use super::Words;

#[test]
fn words_pick_english_only_for_an_english_tag_that_has_words() {
    let words = Words::new("投影", "Schematics");
    assert_eq!(words.get("zh-CN"), "投影");
    assert_eq!(words.get("zh"), "投影");
    assert_eq!(words.get("en"), "Schematics");
    assert_eq!(words.get("en-US"), "Schematics");
    // An English request with no English string falls back to Chinese.
    assert_eq!(Words::new("投影", "").get("en"), "投影");
    assert_eq!(Words::default().get("en"), "");
    assert_eq!(Words::default().get("zh-CN"), "");
}
