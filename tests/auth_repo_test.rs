use sqlx::{PgPool, postgres::PgPoolOptions};

use chatwarp_api::{
    db::auth_repo::AuthRepo,
    wa::auth::AuthState,
};

#[tokio::test]
async fn auth_repo_save_load_roundtrip() -> anyhow::Result<()> {
    let Some(pool) = test_pool().await? else {
        eprintln!("skipping auth_repo_save_load_roundtrip (TEST_DATABASE_URL not set)");
        return Ok(());
    };

    ensure_schema(&pool).await?;

    let repo = AuthRepo::new(pool.clone());
    let instance_name = "test-instance-m4";
    let state = AuthState::new();

    repo.save(instance_name, &state).await?;
    let loaded = repo.load(instance_name).await?;

    assert_eq!(loaded, Some(state));
    Ok(())
}

#[tokio::test]
async fn auth_repo_load_returns_none_when_missing() -> anyhow::Result<()> {
    let Some(pool) = test_pool().await? else {
        eprintln!("skipping auth_repo_load_returns_none_when_missing (TEST_DATABASE_URL not set)");
        return Ok(());
    };

    ensure_schema(&pool).await?;

    let repo = AuthRepo::new(pool);
    let loaded = repo.load("unknown-instance").await?;

    assert!(loaded.is_none());
    Ok(())
}

async fn test_pool() -> anyhow::Result<Option<PgPool>> {
    let url = match std::env::var("TEST_DATABASE_URL") {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let pool = PgPoolOptions::new().max_connections(1).connect(&url).await?;
    Ok(Some(pool))
}

async fn ensure_schema(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS auth_states (
            instance_name TEXT PRIMARY KEY,
            state_json TEXT NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
