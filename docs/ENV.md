# ENV.md

> Variáveis de ambiente consumidas no estado atual do projeto (M0-M4).

## Runtime

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `SERVER_PORT` | `8080` | Não | Porta do servidor HTTP |

## Persistência (M4)

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `TEST_DATABASE_URL` | — | Não | Usada apenas em `tests/auth_repo_test.rs` |

## Notas

- `SERVER_PORT` é a única variável lida no runtime atual.
- O restante das variáveis históricas será reintroduzido conforme M5+.
- Não há dependência de sidecar gRPC nesta fase.
