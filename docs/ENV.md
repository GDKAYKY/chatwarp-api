# ENV.md

> Variáveis de ambiente consumidas no estado atual do projeto (M0-M10).

## Runtime

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `SERVER_PORT` | `8080` | Não | Porta do servidor HTTP |
| `SERVER_BODY_LIMIT_KB` | `256` | Não | Limite máximo de body HTTP por requisição |
| `WA_WS_URL` | `wss://web.whatsapp.com/ws/chat` | Não | Endpoint websocket usado pelo runner de instâncias |
| `WA_RUNNER_MODE` | `wa_rs` | Não | Modo de runner aceito em runtime atual. Valores diferentes de `wa_rs` são inválidos |
| `WA_RS_BOT_COMMAND` | — | Não | Comando shell executado por instância para iniciar o bot wa-rs |
| `WA_RS_AUTH_POLL_INTERVAL_SECS` | `2` | Não | Intervalo de polling do `AuthStore` para sincronizar estado da instância em modo `wa_rs` |
| `DATABASE_URL` | — | Sim (runtime) | DSN PostgreSQL para persistência de autenticação (`auth_states`) |

## Persistência (M4)

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `TEST_DATABASE_URL` | — | Não | Usada apenas em `tests/auth_repo_test.rs` |

## Notas

- Runtime continua sem sidecar gRPC.
- `SERVER_BODY_LIMIT_KB` protege endpoints contra payload excessivo (hardening M10).
- `TEST_DATABASE_URL` continua opcional e usado apenas por testes específicos de repositório.
- Em modo `wa_rs`, mensagens outbound são enfileiradas na tabela `wa_runner_outbox` para consumo do bot.
