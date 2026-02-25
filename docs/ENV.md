# ENV.md

> Variáveis de ambiente consumidas no estado atual do projeto (M0-M2).

## Runtime

| Variável | Default | Obrigatória | Observação |
|---|---|---|---|
| `SERVER_PORT` | `8080` | Não | Porta do servidor HTTP |

## Notas

- `SERVER_PORT` é a única variável lida no runtime atual.
- O restante das variáveis históricas será reintroduzido conforme M3+.
- Não há dependência de sidecar gRPC nesta fase.
