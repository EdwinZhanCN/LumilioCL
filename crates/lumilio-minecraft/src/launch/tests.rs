use super::tokenize_legacy_arguments;

#[test]
fn legacy_tokenizer_supports_both_quote_styles_and_escapes() {
    let tokens = tokenize_legacy_arguments("--first 'two words' --third escaped\\ value").unwrap();

    assert_eq!(tokens, ["--first", "two words", "--third", "escaped value"]);
}
