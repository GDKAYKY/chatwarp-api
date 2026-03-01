use prost::Message;
use serde_json::Value;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::wa::{
    binary_node::{self, NodeContent},
    error::{HandshakeError, HandshakePhase, NoiseError},
    auth::AuthState,
    handshake_proto::HandshakeMessage,
    keys::{KeyPair, generate_keypair},
    noise_md::NoiseMdState,
    noise::NoiseState,
    proto_md::wa::{self, handshake_message},
    transport::WsTransport,
    version::WaWebVersion,
    types::WA_NOISE_PROLOGUE,
};

/// Handshake result used by higher-level connect flows.
#[derive(Debug, Clone)]
pub struct HandshakeOutcome {
    /// Initialized Noise state ready for encrypted session traffic.
    pub noise: NoiseState,
    /// Optional QR reference for first-time login.
    pub qr_reference: Option<String>,
    /// Raw server payload returned after client finish.
    pub server_payload: Vec<u8>,
    /// Optional JID when server confirms resumed login.
    pub login_jid: Option<String>,
    /// Client Noise public key used to build QR payload.
    pub noise_public: [u8; 32],
}

/// Handshake result for real WA MD bootstrap.
#[derive(Debug, Clone)]
pub struct MdHandshakeOutcome {
    /// Initialized MD noise transport state.
    pub noise: NoiseMdState,
    /// Optional QR references extracted from early server stanzas.
    pub qr_references: Vec<String>,
    /// Raw payloads received immediately after client finish.
    pub server_payloads: Vec<Vec<u8>>,
    /// Optional JID when server confirms resumed login.
    pub login_jid: Option<String>,
    /// Client Noise public key used to build QR payload.
    pub noise_public: [u8; 32],
}

/// Performs a Noise XX handshake and returns initialized session data.
pub async fn do_handshake(
    transport: &mut WsTransport,
    static_keypair: &KeyPair,
) -> Result<HandshakeOutcome, HandshakeError> {
    let mut noise = NoiseState::new(WA_NOISE_PROLOGUE);

    let ephemeral = generate_keypair();
    noise.mix_hash(&ephemeral.public);

    let client_hello = HandshakeMessage {
        client_ephemeral: ephemeral.public.to_vec(),
        server_ephemeral: Vec::new(),
        encrypted_static: Vec::new(),
        payload: Vec::new(),
        qr_reference: None,
        login_jid: None,
    };

    let mut encoded_hello = Vec::new();
    client_hello.encode(&mut encoded_hello)?;
    transport
        .send_frame(&encoded_hello)
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientHello, error.to_string()))?;

    let server_hello_frame = transport
        .next_raw_frame()
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ServerHello, error.to_string()))?;
    let server_hello = decode_server_hello_frame(&server_hello_frame)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ServerHello, error.to_string()))?;

    let server_ephemeral = fixed_key(&server_hello.server_ephemeral, "server_ephemeral")?;
    noise.mix_hash(&server_ephemeral);

    let dh1 = diffie_hellman(ephemeral.private, server_ephemeral);
    noise.mix_into_key(&dh1);

    let ad1 = noise.handshake_hash();
    let decrypted_server_static = noise
        .decrypt_with_ad(&server_hello.encrypted_static, &ad1)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ServerHello, error.to_string()))?;
    let server_static = fixed_key(&decrypted_server_static, "server_static")?;

    let dh2 = diffie_hellman(ephemeral.private, server_static);
    noise.mix_into_key(&dh2);

    let ad2 = noise.handshake_hash();
    let encrypted_client_static = noise
        .encrypt_with_ad(&static_keypair.public, &ad2)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientFinish, error.to_string()))?;

    let client_finish = HandshakeMessage {
        client_ephemeral: Vec::new(),
        server_ephemeral: Vec::new(),
        encrypted_static: encrypted_client_static,
        payload: Vec::new(),
        qr_reference: None,
        login_jid: None,
    };

    let mut encoded_finish = Vec::new();
    client_finish.encode(&mut encoded_finish)?;
    transport
        .send_frame(&encoded_finish)
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientFinish, error.to_string()))?;

    // Handshake transitions to encrypted Noise transport after client finish.
    let server_finish_frame = transport
        .next_raw_frame()
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::PostFinish, error.to_string()))?;
    let decrypted = decrypt_server_finish_frame(&mut noise, &server_finish_frame)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::PostFinish, error.to_string()))?;
    let qr_reference =
        extract_qr_reference_from_node(&decrypted).or_else(|| parse_qr_reference(&decrypted));

    Ok(HandshakeOutcome {
        noise,
        qr_reference,
        server_payload: decrypted,
        login_jid: None,
        noise_public: ephemeral.public,
    })
}

/// Performs WA MD Noise XX handshake using Baileys-compatible protobuf payloads.
pub async fn do_handshake_md(
    transport: &mut WsTransport,
    auth: &AuthState,
    version: WaWebVersion,
) -> Result<MdHandshakeOutcome, HandshakeError> {
    let ephemeral = generate_keypair();
    let mut noise = NoiseMdState::new(ephemeral.public, auth.metadata.routing_info.as_deref());

    let client_hello = NoiseMdState::build_client_hello(ephemeral.public);
    let mut client_hello_payload = Vec::new();
    client_hello
        .encode(&mut client_hello_payload)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientHello, error.to_string()))?;

    let framed_client_hello = noise.encode_frame(&client_hello_payload)?;
    transport
        .send_raw(&framed_client_hello)
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientHello, error.to_string()))?;

    let server_hello_raw = transport
        .next_raw_frame()
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ServerHello, error.to_string()))?;
    let server_hello_message = decode_md_server_hello_message(&mut noise, server_hello_raw.as_ref())?;
    let server_hello = server_hello_message
        .server_hello
        .as_ref()
        .ok_or_else(|| HandshakeError::with_phase(HandshakePhase::ServerHello, "missing server_hello"))?;

    let encrypted_static = noise.process_server_hello(server_hello, &auth.noise_key, &ephemeral)?;
    let client_payload = build_client_payload(auth, version)?;
    let mut encoded_client_payload = Vec::new();
    client_payload
        .encode(&mut encoded_client_payload)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientFinish, error.to_string()))?;

    let encrypted_payload = noise.encrypt_handshake_payload(&encoded_client_payload)?;
    let client_finish = wa::HandshakeMessage {
        client_hello: None,
        server_hello: None,
        client_finish: Some(handshake_message::ClientFinish {
            r#static: encrypted_static,
            payload: encrypted_payload,
            extended_ciphertext: Vec::new(),
        }),
    };
    let mut client_finish_payload = Vec::new();
    client_finish
        .encode(&mut client_finish_payload)
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientFinish, error.to_string()))?;

    let framed_client_finish = noise.encode_frame(&client_finish_payload)?;
    transport
        .send_raw(&framed_client_finish)
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::ClientFinish, error.to_string()))?;

    noise.finish_init();

    let mut qr_references = Vec::new();
    let mut server_payloads = Vec::new();
    let mut login_jid = None;

    let first_transport_frame = transport
        .next_raw_frame()
        .await
        .map_err(|error| HandshakeError::with_phase(HandshakePhase::PostFinish, error.to_string()))?;
    let first_payloads = noise.decode_frames(first_transport_frame.as_ref())?;
    for payload in first_payloads {
        if let Some(reference) = extract_qr_reference_from_real_payload(&payload) {
            qr_references.push(reference);
        }
        if login_jid.is_none() {
            login_jid = extract_login_jid_from_real_payload(&payload);
        }
        server_payloads.push(payload);
    }

    Ok(MdHandshakeOutcome {
        noise,
        qr_references,
        server_payloads,
        login_jid,
        noise_public: ephemeral.public,
    })
}

fn decode_md_server_hello_message(
    noise: &mut NoiseMdState,
    raw_frame: &[u8],
) -> Result<wa::HandshakeMessage, HandshakeError> {
    let mut candidates: Vec<Vec<u8>> = Vec::new();
    if let Ok(frames) = noise.decode_frames(raw_frame) {
        candidates.extend(frames);
    }
    candidates.push(raw_frame.to_vec());
    if let Some(unframed) = maybe_unframe(raw_frame) {
        candidates.push(unframed.to_vec());
    }

    for payload in candidates {
        let Ok(message) = wa::HandshakeMessage::decode(payload.as_slice()) else {
            continue;
        };
        if message.server_hello.is_some() {
            return Ok(message);
        }
    }

    Err(HandshakeError::with_phase(
        HandshakePhase::ServerHello,
        format!(
            "unable to decode server_hello (raw_len={}, head={})",
            raw_frame.len(),
            preview_hex(raw_frame, 24)
        ),
    ))
}

fn build_client_payload(auth: &AuthState, version: WaWebVersion) -> Result<wa::ClientPayload, HandshakeError> {
    let browser = &auth.metadata.browser;
    let country_code = auth.metadata.country_code.trim();
    let locale_country = if country_code.is_empty() {
        "US"
    } else {
        country_code
    };

    let user_agent = wa::client_payload::UserAgent {
        platform: wa::client_payload::user_agent::Platform::Web as i32,
        app_version: Some(wa::client_payload::user_agent::AppVersion {
            primary: version.major,
            secondary: version.minor,
            tertiary: version.patch,
            quaternary: 0,
            quinary: 0,
        }),
        mcc: "000".to_owned(),
        mnc: "000".to_owned(),
        os_version: browser.os_version.clone(),
        manufacturer: String::new(),
        device: "Desktop".to_owned(),
        os_build_number: "0.1".to_owned(),
        phone_id: String::new(),
        release_channel: wa::client_payload::user_agent::ReleaseChannel::Release as i32,
        locale_language_iso_639_1: "en".to_owned(),
        locale_country_iso_3166_1_alpha_2: locale_country.to_owned(),
    };

    let web_sub_platform = match browser.os.as_str() {
        "Mac OS" => wa::client_payload::web_info::WebSubPlatform::Darwin as i32,
        "Windows" => wa::client_payload::web_info::WebSubPlatform::Win32 as i32,
        _ => wa::client_payload::web_info::WebSubPlatform::WebBrowser as i32,
    };

    let mut payload = wa::ClientPayload {
        username: 0,
        passive: false,
        user_agent: Some(user_agent),
        web_info: Some(wa::client_payload::WebInfo {
            web_sub_platform,
        }),
        push_name: auth
            .metadata
            .me
            .as_ref()
            .and_then(|me| me.push_name.clone())
            .unwrap_or_else(|| "Chatwarp".to_owned()),
        connect_type: wa::client_payload::ConnectType::WifiUnknown as i32,
        connect_reason: wa::client_payload::ConnectReason::UserActivated as i32,
        device: 0,
        device_pairing_data: None,
        pull: false,
        lid_db_migrated: false,
    };

    if let Some(me) = auth.metadata.me.as_ref() {
        let (username, device) = parse_jid_for_login(&me.jid)
            .ok_or_else(|| HandshakeError::with_phase(HandshakePhase::ClientFinish, "invalid persisted me.jid"))?;
        payload.username = username;
        payload.device = device;
        payload.passive = true;
        payload.pull = true;
        payload.lid_db_migrated = false;
    } else {
        payload.device_pairing_data = Some(build_registration_payload(auth, version));
    }

    Ok(payload)
}

fn build_registration_payload(
    auth: &AuthState,
    version: WaWebVersion,
) -> wa::client_payload::DevicePairingRegistrationData {
    let build_hash = md5::compute(format!("{}.{}.{}", version.major, version.minor, version.patch));
    let device_props = wa::DeviceProps {
        os: auth.metadata.browser.os.clone(),
        version: Some(wa::device_props::AppVersion {
            primary: 10,
            secondary: 15,
            tertiary: 7,
            quaternary: 0,
            quinary: 0,
        }),
        platform_type: wa::device_props::PlatformType::Chrome as i32,
        require_full_sync: false,
        history_sync_config: Some(default_history_sync_config()),
    };
    let mut encoded_device_props = Vec::new();
    let _ = device_props.encode(&mut encoded_device_props);

    wa::client_payload::DevicePairingRegistrationData {
        e_regid: encode_big_endian(auth.identity.registration_id, 4),
        e_keytype: vec![5],
        e_ident: auth.identity.identity_key.public.to_vec(),
        e_skey_id: encode_big_endian(1, 3),
        e_skey_val: auth.identity.signed_pre_key.public.to_vec(),
        e_skey_sig: auth.identity.signed_pre_key_sig.to_vec(),
        build_hash: build_hash.0.to_vec(),
        device_props: encoded_device_props,
    }
}

fn default_history_sync_config() -> wa::device_props::HistorySyncConfig {
    wa::device_props::HistorySyncConfig {
        storage_quota_mb: 10240,
        inline_initial_payload_in_e2ee_msg: true,
        support_call_log_history: false,
        support_bot_user_agent_chat_history: true,
        support_cag_reactions_and_polls: true,
        support_biz_hosted_msg: true,
        support_recent_sync_chunk_message_count_tuning: true,
        support_hosted_group_msg: true,
        support_fbid_bot_chat_history: true,
        support_message_association: true,
        support_group_history: false,
    }
}

fn encode_big_endian(value: u32, width: usize) -> Vec<u8> {
    let mut out = vec![0_u8; width];
    for (index, byte) in out.iter_mut().enumerate() {
        let shift = ((width - 1 - index) * 8) as u32;
        *byte = ((value >> shift) & 0xFF) as u8;
    }
    out
}

fn parse_jid_for_login(jid: &str) -> Option<(u64, u32)> {
    let (user_part, _) = jid.split_once('@')?;
    let (user_raw, device_raw) = if let Some((user, device)) = user_part.split_once(':') {
        (user, Some(device))
    } else {
        (user_part, None)
    };
    let username = user_raw.parse::<u64>().ok()?;
    let device = device_raw.and_then(|value| value.parse::<u32>().ok()).unwrap_or(0);
    Some((username, device))
}

fn extract_qr_reference_from_real_payload(payload: &[u8]) -> Option<String> {
    let node = binary_node::decode_real(payload).ok()?;
    if node.tag == "ref" {
        if let NodeContent::Bytes(bytes) = &node.content {
            return std::str::from_utf8(bytes)
                .ok()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    if node.tag == "iq" {
        let NodeContent::Nodes(children) = &node.content else {
            return None;
        };
        let pair_device = children.iter().find(|child| child.tag == "pair-device")?;
        let NodeContent::Nodes(pair_children) = &pair_device.content else {
            return None;
        };
        let reference = pair_children.iter().find(|child| child.tag == "ref")?;
        if let NodeContent::Bytes(bytes) = &reference.content {
            return std::str::from_utf8(bytes)
                .ok()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    None
}

fn extract_login_jid_from_real_payload(payload: &[u8]) -> Option<String> {
    let node = binary_node::decode_real(payload).ok()?;
    if node.tag != "iq" {
        return None;
    }

    let NodeContent::Nodes(children) = &node.content else {
        return None;
    };

    let pair = children.iter().find(|child| child.tag == "pair-success")?;
    if let Some(jid) = pair.attrs.get("jid") {
        let trimmed = jid.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let device = if let NodeContent::Nodes(pair_children) = &pair.content {
        pair_children.iter().find(|child| child.tag == "device")
    } else {
        None
    }?;
    device.attrs.get("jid").map(ToOwned::to_owned)
}

fn fixed_key(bytes: &[u8], field: &'static str) -> Result<[u8; 32], HandshakeError> {
    if bytes.is_empty() {
        return Err(HandshakeError::MissingField(field));
    }

    if bytes.len() != 32 {
        return Err(HandshakeError::InvalidKeyLength(field));
    }

    let mut out = [0_u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn diffie_hellman(private: [u8; 32], peer_public: [u8; 32]) -> [u8; 32] {
    let private = StaticSecret::from(private);
    let public = PublicKey::from(peer_public);
    private.diffie_hellman(&public).to_bytes()
}

fn parse_qr_reference(payload: &[u8]) -> Option<String> {
    if payload.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_slice::<Value>(payload) {
        for key in ["ref", "reference"] {
            if let Some(reference) = value.get(key).and_then(Value::as_str) {
                let trimmed = reference.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }

    std::str::from_utf8(payload)
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_qr_reference_from_node(payload: &[u8]) -> Option<String> {
    let node = binary_node::decode(payload).ok()?;

    if node.tag == "ref" {
        if let NodeContent::Bytes(bytes) = &node.content {
            return std::str::from_utf8(bytes)
                .ok()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    node.attrs
        .get("ref")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn decode_server_hello_frame(frame: &[u8]) -> Result<HandshakeMessage, HandshakeError> {
    match HandshakeMessage::decode(frame) {
        Ok(message) => Ok(message),
        Err(raw_error) => {
            if let Some(payload) = maybe_unframe(frame) {
                return HandshakeMessage::decode(payload).map_err(HandshakeError::Decode);
            }
            if frame.len() > 3 {
                if let Ok(message) = HandshakeMessage::decode(&frame[3..]) {
                    return Ok(message);
                }
            }
            tracing::debug!(
                frame_len = frame.len(),
                frame_head = %preview_hex(frame, 24),
                "server_hello raw decode failed"
            );
            Err(HandshakeError::Decode(raw_error))
        }
    }
}

fn decrypt_server_finish_frame(
    noise: &mut NoiseState,
    frame: &[u8],
) -> Result<Vec<u8>, HandshakeError> {
    let ad = noise.handshake_hash();

    // Try raw first because WA may deliver handshake payload directly in websocket binary frames.
    let mut raw_noise = noise.clone();
    if let Ok(decrypted) = raw_noise.decrypt_with_ad(frame, &ad) {
        *noise = raw_noise;
        return Ok(decrypted);
    }

    // Fallback for deployments where the same payload still carries a 3-byte frame prefix.
    if let Some(payload) = maybe_unframe(frame) {
        let mut framed_noise = noise.clone();
        if let Ok(decrypted) = framed_noise.decrypt_with_ad(payload, &ad) {
            *noise = framed_noise;
            return Ok(decrypted);
        }
    }

    Err(HandshakeError::Noise(NoiseError::Cipher))
}

fn maybe_unframe(raw: &[u8]) -> Option<&[u8]> {
    if raw.len() < 3 {
        return None;
    }

    let expected_len = ((raw[0] as usize) << 16) | ((raw[1] as usize) << 8) | raw[2] as usize;
    let payload = &raw[3..];
    if payload.len() >= expected_len {
        return Some(&payload[..expected_len]);
    }

    None
}

fn preview_hex(bytes: &[u8], max_len: usize) -> String {
    let take = bytes.len().min(max_len);
    let mut out = String::with_capacity((take * 3).saturating_sub(1));
    for (index, byte) in bytes[..take].iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
