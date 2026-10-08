use crate::tr;
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
    pub skin_site: Option<String>,
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
            SkinChoice::Local { .. } => Some(tr!("account-skin-local")),
            SkinChoice::LittleSkin => Some(tr!("account-skin-littleskin")),
            SkinChoice::Csl { .. } => Some(tr!("account-skin-site")),
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
                (AccountKind::ThirdParty, None) => tr!("account-kind-third-party").to_owned(),
                (AccountKind::Offline, _) => tr!("account-kind-offline").to_owned(),
            };
            Some(AccountRow {
                selected: settings.selected_account.as_deref() == Some(entry.key().as_str()),
                key: entry.key(),
                uuid: id.to_string(),
                custom_id: entry.kind == AccountKind::Offline && entry.uuid.is_some(),
                name: entry.name.clone(),
                microsoft,
                third_party,
                skin_site: entry.server.as_ref().filter(|_| third_party).map(|url| {
                    if url == lumilio_core::LITTLE_SKIN_URL {
                        return "https://littleskin.cn/".into();
                    }
                    url.clone()
                }),
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
        AuthError::Declined => tr!("account-auth-declined").to_owned(),
        AuthError::Expired => tr!("account-auth-expired").to_owned(),
        AuthError::Cancelled => tr!("account-auth-cancelled").to_owned(),
        AuthError::SignInRequired => tr!("account-auth-sign-in-required").to_owned(),
        AuthError::NoXboxAccount => tr!("account-auth-no-xbox").to_owned(),
        AuthError::ChildAccount => tr!("account-auth-child").to_owned(),
        AuthError::XboxUnavailable => tr!("account-auth-xbox-unavailable").to_owned(),
        AuthError::AdultVerificationRequired => tr!("account-auth-adult-verification").to_owned(),
        AuthError::NoGameOwnership => tr!("account-auth-no-game").to_owned(),
        AuthError::ServicesRefused(_) => tr!("account-auth-services-refused").to_owned(),
        AuthError::CredentialStore(_) => tr!("account-auth-credential-store").to_owned(),
        AuthError::Network(_) => tr!("account-auth-network").to_owned(),
        AuthError::Protocol(_) => tr!("account-auth-protocol").to_owned(),
    }
}

/// One sentence about why talking to an authentication server failed. The
/// words follow HMCL's `account.failed.*` strings
/// (`HMCL/src/main/resources/assets/lang/I18N_zh_CN.properties`, Copyright
/// (C) 2026 huangyuhui and contributors, GPL-3.0-or-later; ADR 0011).
#[must_use]
pub fn yggdrasil_message(error: &lumilio_core::YggdrasilError) -> String {
    use lumilio_core::YggdrasilError;
    match error {
        YggdrasilError::Network(_) => tr!("account-yggdrasil-network").to_owned(),
        YggdrasilError::Malformed(_) => tr!("account-yggdrasil-malformed").to_owned(),
        YggdrasilError::InvalidCredentials => tr!("account-yggdrasil-credentials").to_owned(),
        YggdrasilError::SessionExpired => tr!("account-yggdrasil-session-expired").to_owned(),
        YggdrasilError::NoCharacter => tr!("account-yggdrasil-no-character").to_owned(),
        YggdrasilError::CharacterDeleted => tr!("account-yggdrasil-character-deleted").to_owned(),
        YggdrasilError::Remote { kind, message } => {
            let text = message.as_deref().unwrap_or(kind);
            if text.contains("Invalid token") {
                tr!("account-yggdrasil-invalid-token").to_owned()
            } else if text.contains("no longer available") {
                tr!("account-yggdrasil-migrate").to_owned()
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
        SkinError::Io(_) => tr!("account-skin-error-io").to_owned(),
        SkinError::Picture(_) => tr!("account-skin-error-picture").to_owned(),
        SkinError::Network(_) => tr!("account-skin-error-network").to_owned(),
        SkinError::Malformed(_) => tr!("account-skin-error-malformed").to_owned(),
        SkinError::InvalidApi(_) => tr!("account-skin-error-invalid-api").to_owned(),
    }
}

/// What a failed account change looks like in the dialog: one sentence about
/// what to change, and the raw cause behind 技术详情.
pub fn account_failure(error: &lumilio_core::ServiceError) -> (String, String) {
    use lumilio_core::{ServiceError, SettingsError};
    let message = match error {
        ServiceError::Auth(auth) => auth_message(auth),
        ServiceError::SignInRequired(name) => {
            tr!("account-failure-sign-in-required", name = name.as_str())
        }
        ServiceError::Yggdrasil(error) => yggdrasil_message(error),
        ServiceError::Injector(_) => tr!("account-failure-injector").to_owned(),
        ServiceError::Skin(error) => skin_message(error),
        ServiceError::Appearance(error) => {
            use lumilio_core::AppearanceError;
            match error {
                AppearanceError::NoGameOwnership => tr!("account-auth-no-game"),
                AppearanceError::SignInRequired => tr!("account-auth-sign-in-required"),
                AppearanceError::Picture(_) => tr!("account-look-error-picture"),
                AppearanceError::Network(_) => tr!("account-look-error-network"),
                AppearanceError::Protocol(_) => tr!("account-look-error-protocol"),
                AppearanceError::Refused(_) => tr!("account-look-error-refused"),
                AppearanceError::NotMicrosoft => tr!("account-look-error-read-only"),
                AppearanceError::CapeNotOwned => tr!("account-look-error-cape"),
                AppearanceError::Storage(_) => tr!("account-look-error-storage"),
                AppearanceError::UnknownSkin => tr!("account-look-error-missing"),
                AppearanceError::InvalidOrder => tr!("account-look-error-order"),
            }
            .to_owned()
        }
        ServiceError::NoPendingSignIn => tr!("account-failure-no-pending").to_owned(),
        ServiceError::Settings(SettingsError::DuplicateAccount(name)) => {
            tr!("account-failure-duplicate", name = name.as_str())
        }
        ServiceError::Settings(SettingsError::DuplicateUuid(_)) => {
            tr!("account-failure-duplicate-uuid").to_owned()
        }
        ServiceError::Settings(SettingsError::Profile(_)) => {
            tr!("account-failure-profile").to_owned()
        }
        _ => tr!("account-failure-save").to_owned(),
    };
    (message, error.to_string())
}
