use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use x25519_dalek::{PublicKey, StaticSecret};
use curve25519_dalek::{
    constants::ED25519_BASEPOINT_TABLE,
    edwards::CompressedEdwardsY,
    montgomery::MontgomeryPoint,
    scalar::Scalar,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

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

/// Prefix used by WhatsApp/libsignal when signing public keys.
pub const KEY_BUNDLE_TYPE: u8 = 5;

/// Prepends the libsignal key-bundle prefix to a 32-byte public key.
pub fn signal_public_key(public: &[u8; 32]) -> [u8; 33] {
    let mut out = [0_u8; 33];
    out[0] = KEY_BUNDLE_TYPE;
    out[1..].copy_from_slice(public);
    out
}

/// Signs an arbitrary message using a Curve25519-compatible XEdDSA-style flow.
pub fn sign_message(private: [u8; 32], public: [u8; 32], message: &[u8]) -> [u8; 64] {
    let secret = Scalar::from_bytes_mod_order(private);
    let nonce = hash_to_scalar(&[&private, &public, message]);
    let nonce_point = (&nonce * ED25519_BASEPOINT_TABLE).compress().to_bytes();
    let challenge = hash_to_scalar(&[&nonce_point, &public, message]);
    let s = nonce + challenge * secret;

    
    let mut out = [0_u8; 64];
    out[..32].copy_from_slice(&nonce_point);
    out[32..].copy_from_slice(&s.to_bytes());
    out

    //debug
    
}


/// Verifies a Curve25519-compatible XEdDSA-style signature.
pub fn verify_message(public: [u8; 32], message: &[u8], signature: &[u8]) -> bool {
    if signature.len() != 64 {
        return false;
    }

    // Try Ed25519 pure first (used for WA certificates)
    if let Ok(verifying_key) = VerifyingKey::from_bytes(&public) {
        if let Ok(sig) = Signature::from_slice(signature) {
            if verifying_key.verify(message, &sig).is_ok() {
                return true;
            }
        }
    }

    // Fallback to XEdDSA for Curve25519 keys
    let mut r_bytes = [0_u8; 32];
    r_bytes.copy_from_slice(&signature[..32]);
    let Some(r_point) = CompressedEdwardsY(r_bytes).decompress() else {
        return false;
    };

    let mut s_bytes = [0_u8; 32];
    s_bytes.copy_from_slice(&signature[32..]);
    let Some(s) = Option::<Scalar>::from(Scalar::from_canonical_bytes(s_bytes)) else {
        return false;
    };

    let mont = MontgomeryPoint(public);
    let challenge = hash_to_scalar(&[&r_bytes, &public, message]);
    let lhs = &s * ED25519_BASEPOINT_TABLE;
    for sign in [0, 1] {
        let Some(a_point) = mont.to_edwards(sign) else {
            continue;
        };
        let rhs = r_point + (challenge * a_point);
        if lhs == rhs {
            return true;
        }
    }

    false
}

fn hash_to_scalar(parts: &[&[u8]]) -> Scalar {
    let mut hasher = Sha512::new();
    for part in parts {
        hasher.update(part);
    }

    let mut wide = [0_u8; 64];
    wide.copy_from_slice(&hasher.finalize());
    Scalar::from_bytes_mod_order_wide(&wide)
}
