use chatwarp_api::{
    db::auth_store::{AuthStore, InMemoryAuthStore},
    wa::auth::AuthState,
};

#[tokio::test]
async fn in_memory_auth_store_roundtrip() -> anyhow::Result<()> {
    let store = InMemoryAuthStore::new();
    let state = AuthState::new();

    store.save("alpha", &state).await?;
    let loaded = store
        .load("alpha")
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing alpha state"))?;

    assert_eq!(loaded, state);
    Ok(())
}

#[tokio::test]
async fn in_memory_auth_store_keeps_instances_isolated() -> anyhow::Result<()> {
    let store = InMemoryAuthStore::new();
    let alpha = AuthState::new();
    let beta = AuthState::new();

    store.save("alpha", &alpha).await?;
    store.save("beta", &beta).await?;

    let loaded_alpha = store
        .load("alpha")
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing alpha state"))?;
    let loaded_beta = store
        .load("beta")
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing beta state"))?;

    assert_eq!(loaded_alpha, alpha);
    assert_eq!(loaded_beta, beta);
    Ok(())
}
