# ENV.md

> Variáveis de ambiente consumidas no estado atual do projeto (M0-M10).

## Runtime

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `SERVER_PORT` | `8080` | Não | Porta do servidor HTTP |
| `SERVER_BODY_LIMIT_KB` | `256` | Não | Limite máximo de body HTTP por requisição |
| `WA_WS_URL` | `wss://web.whatsapp.com/ws/chat` | Não | Endpoint websocket usado pelo runner de instâncias |
| `WA_NOISE_CERT_ISSUER_KEYS` | hardcoded fallback | Não | Lista de chaves públicas (hex 64 chars) separadas por vírgula para validar certificados Noise intermediários do WA. Exemplo: `142375...ee6b,ab12cd...9876` |
| `DATABASE_URL` | — | Sim (runtime) | DSN PostgreSQL para persistência de autenticação (`auth_states`) |

## Persistência (M4)

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `TEST_DATABASE_URL` | — | Não | Usada apenas em `tests/auth_repo_test.rs` |

## Notas

- Runtime continua sem sidecar gRPC.
- `SERVER_BODY_LIMIT_KB` protege endpoints contra payload excessivo (hardening M10).
- `TEST_DATABASE_URL` continua opcional e usado apenas por testes específicos de repositório.
