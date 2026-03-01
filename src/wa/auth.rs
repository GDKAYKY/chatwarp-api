use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use base64::Engine;

use crate::wa::keys::{
    KeyPair,
    generate_keypair,
    generate_registration_id,
    sign_message,
    signal_public_key,
};

fn default_noise_key() -> KeyPair {
    generate_keypair()
}

fn default_adv_secret_key() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn default_browser_metadata() -> BrowserMetadata {
    BrowserMetadata::default()
}

fn default_country_code() -> String {
    "US".to_owned()
}

/// WhatsApp account information available after successful login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeInfo {
    /// Full JID for the connected account.
    pub jid: String,
    /// Optional profile display name.
    pub push_name: Option<String>,
}

/// Session metadata that evolves after authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Information about the currently logged in account.
    pub me: Option<MeInfo>,
    /// Last routing info advertised by WA edge routing stanza.
    #[serde(default)]
    pub routing_info: Option<Vec<u8>>,
    /// Browser/device metadata sent in client payload.
    #[serde(default = "default_browser_metadata")]
    pub browser: BrowserMetadata,
    /// ISO-3166 alpha-2 country code used in user agent payload.
    #[serde(default = "default_country_code")]
    pub country_code: String,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            me: None,
            routing_info: None,
            browser: BrowserMetadata::default(),
            country_code: default_country_code(),
        }
    }
}

/// Browser/platform tuple used by the WA client payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserMetadata {
    /// OS/platform (example: Mac OS, Windows, Ubuntu).
    pub os: String,
    /// Browser family (example: Chrome).
    pub browser: String,
    /// OS version string.
    pub os_version: String,
}

impl Default for BrowserMetadata {
    fn default() -> Self {
        Self {
            os: "Mac OS".to_owned(),
            browser: "Chrome".to_owned(),
            os_version: "14.4.1".to_owned(),
        }
    }
}

/// Identity and pre-key material persisted per instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityState {
    /// Static identity key used by Signal-compatible flows.
    pub identity_key: KeyPair,
    /// Registration identifier (14-bit) for the account.
    pub registration_id: u32,
    /// Signed pre-key pair.
    pub signed_pre_key: KeyPair,
    /// Signature for the signed pre-key.
    #[serde(with = "serde_sig64")]
    pub signed_pre_key_sig: [u8; 64],
    /// One-time pre-keys consumed during session bootstrap.
    pub one_time_pre_keys: Vec<KeyPair>,
}

/// Full auth state persisted for each instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthState {
    /// Identity-related cryptographic state.
    pub identity: IdentityState,
    /// Static noise key persisted for MD handshake compatibility.
    #[serde(default = "default_noise_key")]
    pub noise_key: KeyPair,
    /// Base64-encoded adv secret used during pairing validation.
    #[serde(default = "default_adv_secret_key")]
    pub adv_secret_key: String,
    /// Session metadata unrelated to cryptographic identity.
    #[serde(default)]
    pub metadata: SessionMetadata,
}

impl AuthState {
    /// Creates a new auth state with generated identity and pre-keys.
    pub fn new() -> Self {
        let identity_key = generate_keypair();
        let signed_pre_key = generate_keypair();
        let signed_pre_key_sig = sign_message(
            identity_key.private,
            identity_key.public,
            &signal_public_key(&signed_pre_key.public),
        );
        let one_time_pre_keys = (0..16).map(|_| generate_keypair()).collect();

        Self {
            identity: IdentityState {
                identity_key,
                registration_id: generate_registration_id(),
                signed_pre_key,
                signed_pre_key_sig,
                one_time_pre_keys,
            },
            noise_key: generate_keypair(),
            adv_secret_key: default_adv_secret_key(),
            metadata: SessionMetadata::default(),
        }
    }
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

mod serde_sig64 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as DeError};

    pub fn serialize<S>(value: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(value)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(D::Error::invalid_length(bytes.len(), &"64 bytes"));
        }

        let mut out = [0_u8; 64];
        out.copy_from_slice(&bytes);
        Ok(out)
    }
}
