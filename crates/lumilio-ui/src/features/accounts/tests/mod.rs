//! Which account the detail shows (`view`), and Accounts' actions with a
//! launcher behind them (`actions`).

mod actions;
mod view;

/// An offline account row.
fn account(name: &str, selected: bool) -> crate::live::AccountRow {
    crate::live::AccountRow {
        key: name.into(),
        name: name.into(),
        uuid: lumilio_core::ProfileId::offline(name).to_string(),
        selected,
        custom_id: false,
        microsoft: false,
        third_party: false,
        skin_site: None,
        kind_text: "离线账户".into(),
        skin: None,
        needs_sign_in: false,
    }
}
