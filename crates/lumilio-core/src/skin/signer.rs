//! The key the local skin server signs textures with. authlib-injector hands
//! the public half to the game, which then accepts the signed properties.
//! HMCL makes a key per run; ours is made once and kept in the launcher's
//! folder, so a launch never waits for key generation.

use std::fs;
use std::io;
use std::path::Path;

use base64::Engine as _;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::rand_core::OsRng;
use rsa::signature::{SignatureEncoding, Signer as _};
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha1_legacy::Sha1;

/// RSA key size of the real key.
pub(crate) const KEY_BITS: usize = 2048;

pub struct Signer {
    key: SigningKey<Sha1>,
    public_pem: String,
}

impl Signer {
    fn from_private(private: RsaPrivateKey) -> io::Result<Self> {
        let public_pem = RsaPublicKey::from(&private)
            .to_public_key_pem(LineEnding::LF)
            .map_err(io::Error::other)?;
        Ok(Self {
            key: SigningKey::<Sha1>::new(private),
            public_pem,
        })
    }

    #[cfg(test)]
    pub fn generate(bits: usize) -> io::Result<Self> {
        Self::from_private(RsaPrivateKey::new(&mut OsRng, bits).map_err(io::Error::other)?)
    }

    /// The key kept at `path`, or a new one written there first. Blocking
    /// (key generation takes a moment).
    pub fn load_or_create(path: &Path, bits: usize) -> io::Result<Self> {
        if let Ok(pem) = fs::read_to_string(path)
            && let Ok(private) = RsaPrivateKey::from_pkcs8_pem(&pem)
        {
            return Self::from_private(private);
        }
        let private = RsaPrivateKey::new(&mut OsRng, bits).map_err(io::Error::other)?;
        let pem = private
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(io::Error::other)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("pem.part");
        fs::write(&temporary, pem.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&temporary, path)?;
        Self::from_private(private)
    }

    /// The public key in PEM, as a Yggdrasil server publishes it.
    #[must_use]
    pub fn public_key_pem(&self) -> &str {
        &self.public_pem
    }

    /// The SHA1withRSA signature of `data`, in base64.
    #[must_use]
    pub fn sign(&self, data: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.key.sign(data.as_bytes()).to_bytes())
    }
}
