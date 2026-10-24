use super::*;

#[test]
fn a_name_must_exist_and_be_new_unless_it_is_the_one_being_renamed() {
    let taken = vec!["生存".to_owned(), "建筑".to_owned()];
    assert_eq!(name_problem("  ", &taken, None), Some("请输入名称".into()));
    assert!(
        name_problem("生存", &taken, None)
            .unwrap()
            .contains("已经有")
    );
    assert!(
        name_problem(" 生存 ", &taken, None).is_some(),
        "spaces are trimmed"
    );
    assert_eq!(
        name_problem("生存", &taken, Some("生存")),
        None,
        "keeping it is fine"
    );
    assert!(name_problem("建筑", &taken, Some("生存")).is_some());
    assert_eq!(name_problem("新的", &taken, None), None);
}
