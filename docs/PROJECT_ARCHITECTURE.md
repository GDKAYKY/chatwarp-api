# Project Architecture

## Goal
Keep modules grouped by responsibility, not by implementation detail (for example, "conn").

## Current Structure

### `src/auth/`
- `handshake.rs`: Noise handshake bootstrap, produces `NoiseSocket`.
- `pair.rs`: QR/device pairing flow.
- `pair_code.rs`: Pair-code authentication flow.
- `prekeys.rs`: prekey fetch/upload helpers used during auth/session setup.
- `store.rs`: local auth persistence helpers.

### `src/client/`
- `sessions.rs`: client session behavior + E2E session deduplication (`SessionManager`).
- `keepalive.rs`: ping/pong keepalive loop for established sessions.
- `context_impl.rs`, `device_registry.rs`, `lid_pn.rs`, `sender_keys.rs`: client runtime behaviors.

### `src/` (root modules)
- `transport.rs`: thin re-exports for `Transport`, `TransportEvent`, `TransportFactory`.
- `http.rs`: thin re-export for `HttpClient`.
- `utils/jid_utils.rs`: `SERVER_JID` cache utility.

## Layering Notes
- Transport primitives (`transport`, `http`) are infrastructure.
- Handshake is authentication/bootstrap (`auth` layer).
- Keepalive and Signal session orchestration are runtime client behavior (`client` layer).
- JID helpers are generic utility and should not define architecture boundaries.

## Compatibility
- Public imports remain stable through `src/lib.rs` re-exports (for example `crate::handshake` now re-exported from `auth::handshake`).
