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
    /// Signed in on an authlib-injector server (LittleSkin and others).
    pub third_party: bool,
    /// What kind of account this is, in words: `离线账户`, `Microsoft`, or the
    /// name of the server it is signed in on.
    pub kind_text: String,
    /// An offline account's skin, when it has one chosen.
    pub skin: Option<lumilio_core::SkinChoice>,
    /// The stored sign-in no longer works.
    pub needs_sign_in: bool,
}

impl AccountRow {
    /// `离线账户`, `Microsoft`, or the authentication server's name.
    #[must_use]
    pub fn kind_label(&self) -> &str {
        &self.kind_text
    }

    /// Whether a sign-in (not an offline name) stands behind this account.
    #[must_use]
    pub const fn signed_in(&self) -> bool {
        self.microsoft || self.third_party
    }

    /// The skin in words, for an offline account that has one.
    #[must_use]
    pub fn skin_text(&self) -> Option<&'static str> {
        use lumilio_core::SkinChoice;
        match self.skin.as_ref()? {
            SkinChoice::Local { .. } => Some("本地皮肤"),
            SkinChoice::LittleSkin => Some("LittleSkin 皮肤"),
            SkinChoice::Csl { .. } => Some("皮肤站皮肤"),
        }
    }
}

/// The name an authentication server is shown by: the one it told us, or its
/// address.
#[must_use]
pub fn server_name(settings: &LauncherSettings, url: &str) -> String {
    if url == lumilio_core::LITTLE_SKIN_URL {
        return "LittleSkin".to_owned();
    }
    settings
        .auth_servers
        .iter()
        .find(|server| server.url == url)
        .and_then(|server| server.name.clone())
        .unwrap_or_else(|| url.to_owned())
}

/// Accounts as the page shows them; an entry whose id cannot be read is left out.
pub fn account_rows(settings: &LauncherSettings) -> Vec<AccountRow> {
    use lumilio_core::AccountKind;
    settings
        .accounts
        .iter()
        .filter_map(|entry| {
            let id = entry.profile_id().ok()?;
            let microsoft = entry.kind == AccountKind::Microsoft;
            let third_party = entry.kind == AccountKind::ThirdParty;
            let kind_text = match (&entry.kind, &entry.server) {
                (AccountKind::Microsoft, _) => "Microsoft".to_owned(),
                (AccountKind::ThirdParty, Some(url)) => server_name(settings, url),
                (AccountKind::ThirdParty, None) => "第三方账户".to_owned(),
                (AccountKind::Offline, _) => "离线账户".to_owned(),
            };
            Some(AccountRow {
                selected: settings.selected_account.as_deref() == Some(entry.key().as_str()),
                key: entry.key(),
                uuid: id.to_string(),
                custom_id: entry.kind == AccountKind::Offline && entry.uuid.is_some(),
                name: entry.name.clone(),
                microsoft,
                third_party,
                kind_text,
                skin: entry.skin.clone(),
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

/// One sentence about why talking to an authentication server failed. The
/// words follow HMCL's (`account.failed.*`).
#[must_use]
pub fn yggdrasil_message(error: &lumilio_core::YggdrasilError) -> String {
    use lumilio_core::YggdrasilError;
    match error {
        YggdrasilError::Network(_) => {
            "无法连接认证服务器。可能是网络问题，请检查设备能否正常上网，或使用代理服务".to_owned()
        }
        YggdrasilError::Malformed(_) => "无法解析认证服务器响应，可能是服务器故障".to_owned(),
        YggdrasilError::InvalidCredentials => {
            "用户名或密码错误，或登录次数过多被暂时禁止登录，请稍后再试".to_owned()
        }
        YggdrasilError::SessionExpired => "登录已经失效，需要重新登录".to_owned(),
        YggdrasilError::NoCharacter => "该账户在这个服务器上没有角色".to_owned(),
        YggdrasilError::CharacterDeleted => "此角色已被删除".to_owned(),
        YggdrasilError::Remote { kind, message } => {
            let text = message.as_deref().unwrap_or(kind);
            if text.contains("Invalid token") {
                "登录已经失效，请重新登录".to_owned()
            } else if text.contains("no longer available") {
                "你的账户需要迁移至微软账户。如果已经迁移，请使用迁移后的微软账户登录".to_owned()
            } else {
                text.to_owned()
            }
        }
    }
}

/// One sentence about why a skin could not be used.
#[must_use]
pub fn skin_message(error: &lumilio_core::SkinError) -> String {
    use lumilio_core::SkinError;
    match error {
        SkinError::Io(_) => "读不到这个皮肤文件".to_owned(),
        SkinError::Picture(_) => "无法识别的皮肤文件，需要是 PNG 图片".to_owned(),
        SkinError::Network(_) => "连不上皮肤站，请检查网络和地址".to_owned(),
        SkinError::Malformed(_) => "皮肤站的回答不符合预期".to_owned(),
        SkinError::InvalidApi(_) => "皮肤站地址不是一个有效的地址".to_owned(),
    }
}

/// What a failed account change looks like in the dialog: one sentence about
/// what to change, and the raw cause behind 技术详情.
pub fn account_failure(error: &lumilio_core::ServiceError) -> (String, String) {
    use lumilio_core::{ServiceError, SettingsError};
    let message = match error {
        ServiceError::Auth(auth) => auth_message(auth),
        ServiceError::SignInRequired(name) => format!("账户 {name} 需要重新登录"),
        ServiceError::Yggdrasil(error) => yggdrasil_message(error),
        ServiceError::Injector(_) => {
            "无法下载 authlib-injector。可能是网络问题，请检查网络、尝试切换下载源或使用代理服务"
                .to_owned()
        }
        ServiceError::Skin(error) => skin_message(error),
        ServiceError::NoPendingSignIn => "这次登录已经失效，请重新登录".to_owned(),
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
