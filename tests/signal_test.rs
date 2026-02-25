use chatwarp_api::wa::signal::{
    InMemorySignalStore,
    SessionBundle,
    decrypt,
    encrypt,
    init_session,
    store::IdentityKeyStore,
};

#[test]
fn signal_roundtrip_between_two_stores() -> anyhow::Result<()> {
    let alice = InMemorySignalStore::from_identity_key([0x11; 32]);
    let bob = InMemorySignalStore::from_identity_key([0x22; 32]);

    let shared_secret = [0xAB; 32];

    let bundle_for_alice = SessionBundle {
        peer_identity_key: bob.local_identity_key()?,
        shared_secret,
        pre_key_id: 1,
        signed_pre_key_id: 1,
    };

    let bundle_for_bob = SessionBundle {
        peer_identity_key: alice.local_identity_key()?,
        shared_secret,
        pre_key_id: 1,
        signed_pre_key_id: 1,
    };

    init_session("bob@s.whatsapp.net", &bundle_for_alice, &alice)?;
    init_session("alice@s.whatsapp.net", &bundle_for_bob, &bob)?;

    let ciphertext = encrypt("bob@s.whatsapp.net", b"hello bob", &alice)?;
    let plaintext = decrypt("alice@s.whatsapp.net", &ciphertext, &bob)?;
    assert_eq!(plaintext, b"hello bob");

    let reply = encrypt("alice@s.whatsapp.net", b"hello alice", &bob)?;
    let reply_plaintext = decrypt("bob@s.whatsapp.net", &reply, &alice)?;
    assert_eq!(reply_plaintext, b"hello alice");

    Ok(())
}

#[test]
fn signal_encrypt_without_session_fails() {
    let store = InMemorySignalStore::new();
    let error = encrypt("nobody@s.whatsapp.net", b"payload", &store).expect_err("must fail");
    assert_eq!(error.to_string(), "missing signal session");
}
