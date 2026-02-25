use chatwarp_api::wa::auth::AuthState;

#[test]
fn auth_state_new_generates_expected_defaults() {
    let state = AuthState::new();

    assert!(state.identity.registration_id < 16_384);
    assert!(!state.identity.one_time_pre_keys.is_empty());
    assert!(state.metadata.me.is_none());
}

#[test]
fn auth_state_serialization_roundtrip() -> anyhow::Result<()> {
    let state = AuthState::new();

    let serialized = serde_json::to_string(&state)?;
    let loaded: AuthState = serde_json::from_str(&serialized)?;

    assert_eq!(state, loaded);
    Ok(())
}
