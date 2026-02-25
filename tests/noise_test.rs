use chatwarp_api::wa::noise::NoiseState;

#[tokio::test]
async fn noise_round_trip_with_incrementing_counters() -> anyhow::Result<()> {
    let ad = tokio::fs::read("tests/fixtures/noise_synthetic/ad.bin").await?;
    let fixture_message = tokio::fs::read("tests/fixtures/noise_synthetic/message.bin").await?;

    let mut sender = NoiseState::new_wa();
    let mut receiver = NoiseState::new_wa();

    let key_material = [0xAB_u8; 32];
    sender.mix_into_key(&key_material);
    receiver.mix_into_key(&key_material);

    for payload in [fixture_message, b"short payload".to_vec(), vec![0x11; 2048]] {
        let ciphertext = sender.encrypt_with_ad(&payload, &ad)?;
        let decrypted = receiver.decrypt_with_ad(&ciphertext, &ad)?;
        assert_eq!(decrypted, payload);
    }

    Ok(())
}

#[tokio::test]
async fn noise_rejects_wrong_ad() -> anyhow::Result<()> {
    let mut sender = NoiseState::new_wa();
    let mut receiver = NoiseState::new_wa();

    let key_material = [0x34_u8; 32];
    sender.mix_into_key(&key_material);
    receiver.mix_into_key(&key_material);

    let ciphertext = sender.encrypt_with_ad(b"ad-protected", b"ad-a")?;
    let decrypted = receiver.decrypt_with_ad(&ciphertext, b"ad-b");

    assert!(decrypted.is_err());
    Ok(())
}

#[tokio::test]
async fn noise_uses_unique_nonces_per_message() -> anyhow::Result<()> {
    let mut sender = NoiseState::new_wa();
    let mut receiver = NoiseState::new_wa();

    let key_material = [0x55_u8; 32];
    sender.mix_into_key(&key_material);
    receiver.mix_into_key(&key_material);

    let ad = b"nonce-check";
    let first = sender.encrypt_with_ad(b"same plaintext", ad)?;
    let second = sender.encrypt_with_ad(b"same plaintext", ad)?;

    assert_ne!(first, second);

    let first_dec = receiver.decrypt_with_ad(&first, ad)?;
    let second_dec = receiver.decrypt_with_ad(&second, ad)?;

    assert_eq!(first_dec, b"same plaintext");
    assert_eq!(second_dec, b"same plaintext");

    Ok(())
}
