use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use prost::Message;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

use crate::wa::{
    error::HandshakeError,
    keys::{KeyPair, verify_message},
    proto_md::wa::{CertChain, HandshakeMessage, cert_chain, handshake_message},
};

const NOISE_MODE: &[u8] = b"Noise_XX_25519_AESGCM_SHA256\0\0\0\0";
const NOISE_WA_HEADER: [u8; 4] = [87, 65, 6, 3];
const WA_CERT_SERIAL: u32 = 0;
const WA_CERT_ISSUER: [u8; 32] = [
    0x14, 0x23, 0x75, 0x57, 0x4d, 0x0a, 0x58, 0x71, 0x66, 0xaa, 0xe7, 0x1e, 0xbe, 0x51, 0x64,
    0x37, 0xc4, 0xa2, 0x8b, 0x73, 0xe3, 0x69, 0x5c, 0x6c, 0xe1, 0xf7, 0xf9, 0x54, 0x5d, 0xa8,
    0xee, 0x6b,
];
const WA_NOISE_CERT_ISSUER_KEYS_ENV: &str = "WA_NOISE_CERT_ISSUER_KEYS";

#[derive(Debug, Clone)]
struct TransportKeys {
    enc_key: [u8; 32],
    dec_key: [u8; 32],
    write_counter: u32,
    read_counter: u32,
}

/// MD noise handler compatible with the WA web transport framing.
#[derive(Debug, Clone)]
pub struct NoiseMdState {
    hash: [u8; 32],
    salt: [u8; 32],
    enc_key: [u8; 32],
    dec_key: [u8; 32],
    counter: u32,
    intro_header: Vec<u8>,
    sent_intro: bool,
    frame_buffer: Vec<u8>,
    transport: Option<TransportKeys>,
}

impl NoiseMdState {
    pub fn new(ephemeral_public: [u8; 32], routing_info: Option<&[u8]>) -> Self {
        let hash = initialize_handshake_hash(NOISE_MODE);

        let intro_header = build_intro_header(routing_info);

        let mut state = Self {
            hash,
            salt: hash,
            enc_key: hash,
            dec_key: hash,
            counter: 0,
            intro_header,
            sent_intro: false,
            frame_buffer: Vec::new(),
            transport: None,
        };

        state.authenticate(&NOISE_WA_HEADER);
        state.authenticate(&ephemeral_public);
        state
    }

    pub fn build_client_hello(ephemeral_public: [u8; 32]) -> HandshakeMessage {
        HandshakeMessage {
            client_hello: Some(handshake_message::ClientHello {
                ephemeral: ephemeral_public.to_vec(),
                r#static: Vec::new(),
                payload: Vec::new(),
                use_extended: false,
                extended_ciphertext: Vec::new(),
            }),
            server_hello: None,
            client_finish: None,
        }
    }

    pub fn process_server_hello(
        &mut self,
        server_hello: &handshake_message::ServerHello,
        noise_key: &KeyPair,
        ephemeral_key: &KeyPair,
    ) -> Result<Vec<u8>, HandshakeError> {
        self.authenticate(&server_hello.ephemeral);

        let server_ephemeral = to_32(&server_hello.ephemeral, "server_hello.ephemeral")?;
        let dh_ephemeral = diffie_hellman(ephemeral_key.private, server_ephemeral);
        self.mix_into_key(&dh_ephemeral);

        let static_ciphertext = if !server_hello.r#static.is_empty() {
            server_hello.r#static.as_slice()
        } else {
            server_hello.extended_static.as_slice()
        };
        if static_ciphertext.is_empty() {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "missing server_hello.static",
            ));
        }
        let decrypted_static = self.decrypt_handshake(static_ciphertext).map_err(|_| {
            HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                format!(
                    "decrypt failed at server_hello.static (ephemeral_len={}, static_len={}, extended_static_len={}, payload_len={})",
                    server_hello.ephemeral.len(),
                    server_hello.r#static.len(),
                    server_hello.extended_static.len(),
                    server_hello.payload.len()
                ),
            )
        })?;
        let server_static = to_32(&decrypted_static, "server_hello.static")?;

        let dh_static = diffie_hellman(ephemeral_key.private, server_static);
        self.mix_into_key(&dh_static);

        let cert_decoded = self.decrypt_handshake(&server_hello.payload).map_err(|_| {
            HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                format!(
                    "decrypt failed at server_hello.payload (ephemeral_len={}, static_len={}, extended_static_len={}, payload_len={})",
                    server_hello.ephemeral.len(),
                    server_hello.r#static.len(),
                    server_hello.extended_static.len(),
                    server_hello.payload.len()
                ),
            )
        })?;
        self.verify_cert_chain(&cert_decoded)?;

        let key_enc = self.encrypt_handshake(&noise_key.public)?;
        let dh_noise = diffie_hellman(noise_key.private, server_ephemeral);
        self.mix_into_key(&dh_noise);

        Ok(key_enc)
    }

    pub fn finish_init(&mut self) {
        let (write, read) = self.local_hkdf(&[]);
        self.transport = Some(TransportKeys {
            enc_key: write,
            dec_key: read,
            write_counter: 0,
            read_counter: 0,
        });
    }

    pub fn encode_frame(&mut self, data: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let payload = if self.transport.is_some() {
            self.encrypt_transport(data)?
        } else {
            data.to_vec()
        };

        if payload.len() > 0xFF_FF_FF {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::PostFinish,
                "payload too large for 24-bit frame",
            ));
        }

        let intro_size = if self.sent_intro { 0 } else { self.intro_header.len() };
        let len = payload.len();
        let mut out = Vec::with_capacity(intro_size + 3 + len);
        if !self.sent_intro {
            out.extend_from_slice(&self.intro_header);
            self.sent_intro = true;
        }

        out.push(((len >> 16) & 0xFF) as u8);
        out.push(((len >> 8) & 0xFF) as u8);
        out.push((len & 0xFF) as u8);
        out.extend_from_slice(&payload);
        Ok(out)
    }

    pub fn decode_frames(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, HandshakeError> {
        if chunk.is_empty() {
            return Ok(Vec::new());
        }

        self.frame_buffer.extend_from_slice(chunk);
        let mut out = Vec::new();

        loop {
            if self.frame_buffer.len() < 3 {
                break;
            }

            let expected_len = ((self.frame_buffer[0] as usize) << 16)
                | ((self.frame_buffer[1] as usize) << 8)
                | self.frame_buffer[2] as usize;
            let full_len = 3 + expected_len;
            if self.frame_buffer.len() < full_len {
                break;
            }

            let payload = self.frame_buffer[3..full_len].to_vec();
            self.frame_buffer.drain(..full_len);

            if self.transport.is_some() {
                out.push(self.decrypt_transport(&payload)?);
            } else {
                out.push(payload);
            }
        }

        Ok(out)
    }

    pub fn decrypt_handshake_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        self.decrypt_handshake(payload)
    }

    pub fn encrypt_handshake_payload(&mut self, payload: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        self.encrypt_handshake(payload)
    }

    fn verify_cert_chain(&self, cert_payload: &[u8]) -> Result<(), HandshakeError> {
        let cert_chain = CertChain::decode(cert_payload)
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::ServerHello, error.to_string()))?;

        let Some(intermediate) = cert_chain.intermediate else {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "missing intermediate cert",
            ));
        };

        let intermediate_signature = intermediate.signature.as_slice();
        if intermediate_signature.is_empty() {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "missing intermediate cert signature",
            ));
        }

        // Verify intermediate signature against RAW details bytes
        let trusted_issuer_keys = trusted_issuer_keys();
        
        tracing::info!(
            issuer_keys_count = trusted_issuer_keys.len(),
            details_len = intermediate.details.len(),
            signature_len = intermediate_signature.len(),
            "verifying intermediate certificate"
        );
        
        let verified_intermediate = trusted_issuer_keys
            .iter()
            .any(|issuer_key| verify_message(*issuer_key, intermediate.details.as_slice(), intermediate_signature));
        if !verified_intermediate {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                format!(
                    "noise intermediate certificate signature invalid (trusted_issuer_keys={}, details_len={}, sig_len={})",
                    trusted_issuer_keys.len(),
                    intermediate.details.len(),
                    intermediate_signature.len()
                ),
            ));
        }

        // Now decode details for validation
        let details = cert_chain::noise_certificate::Details::decode(intermediate.details.as_slice())
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::ServerHello, error.to_string()))?;

        if details.issuer_serial != WA_CERT_SERIAL {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                format!("unexpected cert issuer serial {}", details.issuer_serial),
            ));
        }

        let Some(leaf) = cert_chain.leaf else {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "missing leaf cert",
            ));
        };
        if leaf.details.is_empty() || leaf.signature.is_empty() {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "invalid noise leaf certificate",
            ));
        }

        let leaf_details = cert_chain::noise_certificate::Details::decode(leaf.details.as_slice())
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::ServerHello, error.to_string()))?;
        if leaf_details.issuer_serial != details.serial {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                format!(
                    "noise leaf certificate chain mismatch (leaf_issuer_serial={}, intermediate_serial={})",
                    leaf_details.issuer_serial, details.serial
                ),
            ));
        }

        let leaf_issuer_key = to_32(&details.key, "intermediate.details.key")?;
        if !verify_message(leaf_issuer_key, leaf.details.as_slice(), leaf.signature.as_slice()) {
            return Err(HandshakeError::with_phase(
                crate::wa::HandshakePhase::ServerHello,
                "noise leaf certificate signature invalid",
            ));
        }

        Ok(())
    }

    fn authenticate(&mut self, bytes: &[u8]) {
        if self.transport.is_none() {
            let mut hasher = Sha256::new();
            hasher.update(self.hash);
            hasher.update(bytes);
            self.hash.copy_from_slice(&hasher.finalize());
        }
    }

    fn local_hkdf(&self, ikm: &[u8]) -> ([u8; 32], [u8; 32]) {
        let hk = Hkdf::<Sha256>::new(Some(&self.salt), ikm);
        let mut output = [0_u8; 64];
        hk.expand(&[], &mut output)
            .expect("hkdf expand should never fail for fixed output size");

        let mut write = [0_u8; 32];
        write.copy_from_slice(&output[..32]);
        let mut read = [0_u8; 32];
        read.copy_from_slice(&output[32..]);
        (write, read)
    }

    fn mix_into_key(&mut self, ikm: &[u8]) {
        let (write, read) = self.local_hkdf(ikm);
        self.salt = write;
        self.enc_key = read;
        self.dec_key = read;
        self.counter = 0;
    }

    fn encrypt_handshake(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let encrypted = aes_encrypt(plaintext, &self.enc_key, self.counter, &self.hash)
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::ServerHello, error))?;
        self.counter = self.counter.wrapping_add(1);
        self.authenticate(&encrypted);
        Ok(encrypted)
    }

    fn decrypt_handshake(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let decrypted = aes_decrypt(ciphertext, &self.dec_key, self.counter, &self.hash)
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::ServerHello, error))?;
        self.counter = self.counter.wrapping_add(1);
        self.authenticate(ciphertext);
        Ok(decrypted)
    }

    fn encrypt_transport(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let Some(transport) = self.transport.as_mut() else {
            return Ok(plaintext.to_vec());
        };

        let encrypted = aes_encrypt(plaintext, &transport.enc_key, transport.write_counter, &[])
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::PostFinish, error))?;
        transport.write_counter = transport.write_counter.wrapping_add(1);
        Ok(encrypted)
    }

    fn decrypt_transport(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let Some(transport) = self.transport.as_mut() else {
            return Ok(ciphertext.to_vec());
        };

        let decrypted = aes_decrypt(ciphertext, &transport.dec_key, transport.read_counter, &[])
            .map_err(|error| HandshakeError::with_phase(crate::wa::HandshakePhase::PostFinish, error))?;
        transport.read_counter = transport.read_counter.wrapping_add(1);
        Ok(decrypted)
    }
}

fn build_intro_header(routing_info: Option<&[u8]>) -> Vec<u8> {
    if let Some(routing_info) = routing_info {
        let mut out = Vec::with_capacity(7 + routing_info.len() + NOISE_WA_HEADER.len());
        out.extend_from_slice(b"ED");
        out.push(0);
        out.push(1);
        out.push(((routing_info.len() >> 16) & 0xFF) as u8);
        out.push(((routing_info.len() >> 8) & 0xFF) as u8);
        out.push((routing_info.len() & 0xFF) as u8);
        out.extend_from_slice(routing_info);
        out.extend_from_slice(&NOISE_WA_HEADER);
        return out;
    }

    NOISE_WA_HEADER.to_vec()
}

fn trusted_issuer_keys() -> &'static Vec<[u8; 32]> {
    static KEYS: OnceLock<Vec<[u8; 32]>> = OnceLock::new();
    KEYS.get_or_init(|| {
        let mut keys = Vec::new();
        if let Ok(raw) = std::env::var(WA_NOISE_CERT_ISSUER_KEYS_ENV) {
            for piece in raw.split(',') {
                let item = piece.trim();
                if item.is_empty() {
                    continue;
                }
                let hex = item.strip_prefix("0x").unwrap_or(item);
                if let Some(key) = parse_hex_32(hex) {
                    keys.push(key);
                } else {
                    tracing::warn!(
                        value = item,
                        "invalid entry in WA_NOISE_CERT_ISSUER_KEYS; expected 32-byte hex key"
                    );
                }
            }
        }

        if keys.is_empty() {
            keys.push(WA_CERT_ISSUER);
        }
        keys
    })
}

fn parse_hex_32(input: &str) -> Option<[u8; 32]> {
    if input.len() != 64 {
        return None;
    }

    let mut out = [0_u8; 32];
    for (idx, byte) in out.iter_mut().enumerate() {
        let start = idx * 2;
        let end = start + 2;
        *byte = u8::from_str_radix(&input[start..end], 16).ok()?;
    }
    Some(out)
}

fn to_32(bytes: &[u8], label: &'static str) -> Result<[u8; 32], HandshakeError> {
    if bytes.len() != 32 {
        return Err(HandshakeError::with_phase(
            crate::wa::HandshakePhase::ServerHello,
            format!("invalid key length for {label}: {}", bytes.len()),
        ));
    }

    let mut out = [0_u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn diffie_hellman(private: [u8; 32], peer_public: [u8; 32]) -> [u8; 32] {
    use x25519_dalek::{PublicKey, StaticSecret};

    let private = StaticSecret::from(private);
    let public = PublicKey::from(peer_public);
    private.diffie_hellman(&public).to_bytes()
}

fn aes_encrypt(plaintext: &[u8], key: &[u8; 32], counter: u32, ad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "cipher init failed".to_owned())?;
    let nonce = build_nonce(counter);
    cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: ad,
            },
        )
        .map_err(|_| "encrypt failed".to_owned())
}

fn aes_decrypt(ciphertext: &[u8], key: &[u8; 32], counter: u32, ad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "cipher init failed".to_owned())?;
    let nonce = build_nonce(counter);
    cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: ciphertext,
                aad: ad,
            },
        )
        .map_err(|_| "decrypt failed".to_owned())
}

fn build_nonce(counter: u32) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[8..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

fn initialize_handshake_hash(protocol_name: &[u8]) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    if protocol_name.len() <= hash.len() {
        hash[..protocol_name.len()].copy_from_slice(protocol_name);
        return hash;
    }

    hash.copy_from_slice(&Sha256::digest(protocol_name));
    hash
}

#[cfg(test)]
mod tests {
    use super::NoiseMdState;

    #[test]
    fn intro_header_without_routing_uses_wa_prefix() {
        let mut state = NoiseMdState::new([1_u8; 32], None);
        let encoded = state.encode_frame(b"abc").expect("encode");
        assert_eq!(&encoded[..4], &[87, 65, 6, 3]);
        assert_eq!(&encoded[4..7], &[0, 0, 3]);
    }

    #[test]
    fn intro_header_with_routing_uses_ed_prefix() {
        let mut state = NoiseMdState::new([2_u8; 32], Some(&[9, 8, 7, 6]));
        let encoded = state.encode_frame(b"x").expect("encode");
        assert_eq!(&encoded[..2], b"ED");
        assert_eq!(&encoded[4..7], &[0, 0, 4]);
    }
}
