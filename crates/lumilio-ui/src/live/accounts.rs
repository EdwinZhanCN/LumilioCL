use lumilio_core::LauncherSettings;

/// One account of the Accounts page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRow {
    /// What selection and removal name this account by (not its display name).
    pub key: String,
    pub name: String,
    /// The dashed profile id the game sees.
    pub uuid: String,
    pub selected: bool,
    /// The id was chosen by the user, not derived from the name.
    pub custom_id: bool,
    /// Signed in with Microsoft rather than offline.
    pub microsoft: bool,
    /// The stored sign-in no longer works.
    pub needs_sign_in: bool,
}

impl AccountRow {
    /// `离线账户` or `Microsoft`.
    #[must_use]
    pub const fn kind_label(&self) -> &'static str {
        if self.microsoft {
            "Microsoft"
        } else {
            "离线账户"
        }
    }
}

/// The rows of the Accounts page, in the order the accounts were added.
pub fn account_rows(settings: &LauncherSettings) -> Vec<AccountRow> {
    settings
        .accounts
        .iter()
        .filter_map(|entry| {
            let id = entry.profile_id().ok()?;
            let microsoft = entry.kind == lumilio_core::AccountKind::Microsoft;
            Some(AccountRow {
                selected: settings.selected_account.as_deref() == Some(entry.key().as_str()),
                key: entry.key(),
                uuid: id.to_string(),
                custom_id: !microsoft && entry.uuid.is_some(),
                name: entry.name.clone(),
                microsoft,
                needs_sign_in: entry.needs_sign_in,
            })
        })
        .collect()
}

/// One sentence about why a Microsoft sign-in step failed, and what to do.
#[must_use]
pub fn auth_message(error: &lumilio_core::AuthError) -> String {
    use lumilio_core::AuthError;
    match error {
        AuthError::Declined => "你在浏览器里拒绝了这次登录".to_owned(),
        AuthError::Expired => "代码已经过期，请重新开始登录".to_owned(),
        AuthError::Cancelled => "登录已取消".to_owned(),
        AuthError::SignInRequired => "登录已失效，需要重新登录".to_owned(),
        AuthError::NoXboxAccount => {
            "这个 Microsoft 账户还没有 Xbox 档案，请先在 xbox.com 创建一个".to_owned()
        }
        AuthError::ChildAccount => {
            "这是儿童账户，需要家长在 Microsoft 家庭组里允许在线游戏".to_owned()
        }
        AuthError::XboxUnavailable => "你所在的地区不提供 Xbox 服务".to_owned(),
        AuthError::AdultVerificationRequired => {
            "这个账户需要先在 Xbox 网站完成成年人验证".to_owned()
        }
        AuthError::NoGameOwnership => "这个账户没有 Minecraft Java 版".to_owned(),
        AuthError::ServicesRefused(_) => {
            "Minecraft 服务拒绝了这个启动器的登录，可能这个应用注册还没有通过 Mojang 的审批"
                .to_owned()
        }
        AuthError::CredentialStore(_) => {
            "系统凭据库不可用，登录信息无法安全保存，所以没有登录".to_owned()
        }
        AuthError::Network(_) => "连不上登录服务，请检查网络后重试".to_owned(),
        AuthError::Protocol(_) => "登录服务的回答不符合预期".to_owned(),
    }
}

/// What a failed account change looks like in the dialog: one sentence about
/// what to change, and the raw cause behind 技术详情.
pub fn account_failure(error: &lumilio_core::ServiceError) -> (String, String) {
    use lumilio_core::{ServiceError, SettingsError};
    let message = match error {
        ServiceError::Auth(auth) => auth_message(auth),
        ServiceError::SignInRequired(name) => {
            format!("账户 {name} 需要重新登录 Microsoft")
        }
        ServiceError::Settings(SettingsError::DuplicateAccount(name)) => {
            format!("已经有叫“{name}”的账户了（名称不分大小写）")
        }
        ServiceError::Settings(SettingsError::DuplicateUuid(_)) => {
            "这个 UUID 已经被另一个账户使用".to_owned()
        }
        ServiceError::Settings(SettingsError::Profile(_)) => "名称或 UUID 不符合要求".to_owned(),
        _ => "没能保存账户".to_owned(),
    };
    (message, error.to_string())
}
