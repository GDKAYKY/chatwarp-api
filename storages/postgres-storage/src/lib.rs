//! PostgreSQL storage backend for chatwarp-api
//!
//! This crate provides a PostgreSQL-based storage implementation for the chatwarp-api library.
//! It implements all the required storage traits from warp_core::store::traits.

mod postgres_store;
mod schema;

pub use postgres_store::PostgresStore;
