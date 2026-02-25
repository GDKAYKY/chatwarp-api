use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::wa::keys::{KeyPair, generate_keypair, generate_registration_id};

/// WhatsApp account information available after successful login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeInfo {
    /// Full JID for the connected account.
    pub jid: String,
    /// Optional profile display name.
    pub push_name: Option<String>,
}

/// Session metadata that evolves after authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionMetadata {
    /// Information about the currently logged in account.
    pub me: Option<MeInfo>,
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
    /// Session metadata unrelated to cryptographic identity.
    pub metadata: SessionMetadata,
}

impl AuthState {
    /// Creates a new auth state with generated identity and pre-keys.
    pub fn new() -> Self {
        let mut signed_pre_key_sig = [0_u8; 64];
        OsRng.fill_bytes(&mut signed_pre_key_sig);

        let one_time_pre_keys = (0..16).map(|_| generate_keypair()).collect();

        Self {
            identity: IdentityState {
                identity_key: generate_keypair(),
                registration_id: generate_registration_id(),
                signed_pre_key: generate_keypair(),
                signed_pre_key_sig,
                one_time_pre_keys,
            },
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
