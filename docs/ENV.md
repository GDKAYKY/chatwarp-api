# Variáveis de Ambiente

Abaixo estão as variáveis atualmente consumidas pela implementação Rust.

## Server
- `SERVER_NAME` (default: `evolution`)
- `SERVER_TYPE` (`http`/`https`, default: `http`)
- `SERVER_PORT` (default: `8080`)
- `SERVER_URL`
- `SERVER_DISABLE_DOCS` (`true`/`false`)
- `SERVER_DISABLE_MANAGER` (`true`/`false`)

## CORS
- `CORS_ORIGIN` (csv, default: `*`)
- `CORS_METHODS` (csv, default: `POST,GET,PUT,DELETE`)
- `CORS_CREDENTIALS` (`true`/`false`)

## TLS
- `SSL_CONF_PRIVKEY`
- `SSL_CONF_FULLCHAIN`

## Provider
- `PROVIDER_ENABLED`
- `PROVIDER_HOST`
- `PROVIDER_PORT` (default: `5656`)
- `PROVIDER_PREFIX` (default: `evolution`)

## Database
- `DATABASE_CONNECTION_URI` (obrigatória)
- `DATABASE_CONNECTION_CLIENT_NAME` (default: `evolution`)
- `DATABASE_PROVIDER` (default: `postgresql`)
- `DATABASE_SAVE_DATA_INSTANCE`

## Auth
- `AUTHENTICATION_API_KEY` (default: `BQYHJGJHJ`)
- `AUTHENTICATION_EXPOSE_IN_FETCH_INSTANCES`

## Sidecar gRPC
- `SIDECAR_GRPC_ENDPOINT` (default: `http://127.0.0.1:50051`)
- `SIDECAR_CONNECT_TIMEOUT_MS` (default: `3000`)

## Events / Metrics / Observability
- `WEBHOOK_EVENTS_ERRORS`
- `WEBHOOK_EVENTS_ERRORS_WEBHOOK`
- `WEBSOCKET_ENABLED`
- `WEBSOCKET_GLOBAL_EVENTS`
- `RABBITMQ_ENABLED`
- `RABBITMQ_GLOBAL_ENABLED`
- `RABBITMQ_URI`
- `RABBITMQ_EXCHANGE_NAME` (default: `evolution_exchange`)
- `PROMETHEUS_METRICS`
- `METRICS_AUTH_REQUIRED`
- `METRICS_USER`
- `METRICS_PASSWORD`
- `METRICS_ALLOWED_IPS` (csv de IPs)
- `SENTRY_DSN`
- `TELEMETRY_ENABLED` (default efetivo: `true` quando ausente)

## Facebook (endpoint `/verify-creds`)
- `FACEBOOK_APP_ID`
- `FACEBOOK_CONFIG_ID`
- `FACEBOOK_USER_TOKEN`
