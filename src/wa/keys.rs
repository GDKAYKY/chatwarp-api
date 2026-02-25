use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

/// X25519 keypair used in WA handshake and identity primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPair {
    /// Public key bytes.
    pub public: [u8; 32],
    /// Private key bytes.
    pub private: [u8; 32],
}

impl KeyPair {
    /// Builds a keypair from a private key.
    pub fn from_private(private: [u8; 32]) -> Self {
        let secret = StaticSecret::from(private);
        let public = PublicKey::from(&secret).to_bytes();

        Self { public, private }
    }

    /// Returns a StaticSecret view of the private key.
    pub fn as_static_secret(&self) -> StaticSecret {
        StaticSecret::from(self.private)
    }
}

/// Generates a new random X25519 keypair.
pub fn generate_keypair() -> KeyPair {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret).to_bytes();

    KeyPair {
        public,
        private: secret.to_bytes(),
    }
}

/// Generates a 14-bit registration identifier.
pub fn generate_registration_id() -> u32 {
    let mut raw = [0_u8; 4];
    OsRng.fill_bytes(&mut raw);
    u32::from_le_bytes(raw) & 0x3FFF
}
