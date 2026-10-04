use super::jobs::outcome;
use lumilio_ui::toast::Toast;

#[test]
fn a_failed_operation_says_it_plainly_and_keeps_the_cause_behind_details() {
    let failed = outcome(
        Err::<String, _>("os error 28: no space left"),
        |file| format!("已装好 {file}"),
        "没有装上 Sodium".to_owned(),
    );
    assert_eq!(
        failed,
        Toast::error("没有装上 Sodium").technical("os error 28: no space left")
    );
    assert!(
        !failed.text.contains("os error"),
        "no raw error as headline"
    );
    let done = outcome(
        Ok::<_, String>("sodium.jar"),
        |file| format!("已装好 {file}"),
        String::new(),
    );
    assert_eq!(done, Toast::success("已装好 sodium.jar"));
}
