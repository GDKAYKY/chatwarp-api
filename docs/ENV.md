# ENV.md

> Variáveis de ambiente consumidas pela implementação Rust.
> Valores sem default são obrigatórios ou ficam desabilitados quando ausentes.

---

## Server

| Variável | Default | Valores |
|---|---|---|
| `SERVER_NAME` | `evolution` | string |
| `SERVER_TYPE` | `http` | `http` / `https` |
| `SERVER_PORT` | `8080` | número |
| `SERVER_URL` | — | URL base pública |
| `SERVER_DISABLE_DOCS` | `false` | `true` / `false` |
| `SERVER_DISABLE_MANAGER` | `false` | `true` / `false` |

---

## CORS

| Variável | Default | Valores |
|---|---|---|
| `CORS_ORIGIN` | `*` | CSV de origens |
| `CORS_METHODS` | `POST,GET,PUT,DELETE` | CSV de métodos |
| `CORS_CREDENTIALS` | — | `true` / `false` |

---

## TLS

Requeridas quando `SERVER_TYPE=https`.

| Variável | Default |
|---|---|
| `SSL_CONF_PRIVKEY` | — |
| `SSL_CONF_FULLCHAIN` | — |

---

## Database

| Variável | Default | Observação |
|---|---|---|
| `DATABASE_CONNECTION_URI` | — | **Obrigatória** |
| `DATABASE_PROVIDER` | `postgresql` | |
| `DATABASE_CONNECTION_CLIENT_NAME` | `warp` | |
| `DATABASE_SAVE_DATA_INSTANCE` | — | |

---

## Auth

| Variável | Default | Observação |
|---|---|---|
| `AUTHENTICATION_API_KEY` | `BQYHJGJHJ` | Trocar em produção |
| `AUTHENTICATION_EXPOSE_IN_FETCH_INSTANCES` | — | |

---

## Sidecar gRPC

| Variável | Default |
|---|---|
| `SIDECAR_GRPC_ENDPOINT` | `http://127.0.0.1:50051` |
| `SIDECAR_CONNECT_TIMEOUT_MS` | `3000` |

---

## Provider

| Variável | Default |
|---|---|
| `PROVIDER_ENABLED` | — |
| `PROVIDER_HOST` | — |
| `PROVIDER_PORT` | `5656` |
| `PROVIDER_PREFIX` | `warp` |

---

## WebSocket

| Variável | Default |
|---|---|
| `WEBSOCKET_ENABLED` | — |
| `WEBSOCKET_GLOBAL_EVENTS` | — |

---

## RabbitMQ

| Variável | Default |
|---|---|
| `RABBITMQ_ENABLED` | — |
| `RABBITMQ_GLOBAL_ENABLED` | — |
| `RABBITMQ_URI` | — |
| `RABBITMQ_EXCHANGE_NAME` | `evolution_exchange` |

---

## Prometheus

| Variável | Default |
|---|---|
| `PROMETHEUS_METRICS` | — |
| `METRICS_AUTH_REQUIRED` | — |
| `METRICS_USER` | — |
| `METRICS_PASSWORD` | — |
| `METRICS_ALLOWED_IPS` | — | CSV de IPs |

---

## Observabilidade

| Variável | Default | Observação |
|---|---|---|
| `SENTRY_DSN` | — | |
| `TELEMETRY_ENABLED` | `true` | Ativo por default quando ausente |
| `WEBHOOK_EVENTS_ERRORS` | — | |
| `WEBHOOK_EVENTS_ERRORS_WEBHOOK` | — | URL de destino dos erros |

---

## Facebook

Requeridas para o endpoint `POST /verify-creds`.

| Variável | Default |
|---|---|
| `FACEBOOK_APP_ID` | — |
| `FACEBOOK_CONFIG_ID` | — |
| `FACEBOOK_USER_TOKEN` | — |