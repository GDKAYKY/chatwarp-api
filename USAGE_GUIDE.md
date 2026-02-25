# USAGE GUIDE

Guia rápido para usar o `chatwarp-api` em outro projeto, especialmente Java.

## 1. Subir a API com Docker Compose

Na raiz deste repositório:

```bash
docker compose up -d --build
```

Validar se está online:

```bash
curl -fsS http://localhost:8080/healthz
```

Resposta esperada:

```json
{"ok":true}
```

## 2. URLs de acesso

- Host local: `http://localhost:8080`
- Dentro da rede Docker (`chatwarp-net`): `http://chatwarp-api:8080`

## 3. Integrar no projeto Java

Se o Java roda em container, coloque ele na mesma rede (`chatwarp-net`).

Exemplo de `docker-compose.yml` no projeto Java:

```yaml
services:
  my-java-app:
    image: my-java-app:latest
    environment:
      CHATWARP_BASE_URL: http://chatwarp-api:8080
    networks:
      - chatwarp-net

networks:
  chatwarp-net:
    external: true
```

Se o Java roda fora de container, use:

- `CHATWARP_BASE_URL=http://localhost:8080`

## 4. Fluxo mínimo de uso da API

1. Criar instância:

```bash
curl -X POST http://localhost:8080/instance/create \
  -H "Content-Type: application/json" \
  -d '{"name":"default","auto_connect":false}'
```

2. Conectar instância (retorna QR quando aplicável):

```bash
curl http://localhost:8080/instance/connect/default
```

3. Ver estado da conexão:

```bash
curl http://localhost:8080/instance/connectionState/default
```

4. Enviar mensagem de texto:

```bash
curl -X POST http://localhost:8080/message/sendText/default \
  -H "Content-Type: application/json" \
  -d '{"to":"5511999999999@s.whatsapp.net","content":{"type":"text","text":"hello from java integration"}}'
```

## 5. Exemplo Java (Spring WebClient)

```java
import org.springframework.web.reactive.function.client.WebClient;
import java.util.Map;

public class ChatwarpClient {
    private final WebClient client;

    public ChatwarpClient(String baseUrl) {
        this.client = WebClient.builder()
                .baseUrl(baseUrl)
                .build();
    }

    public String health() {
        return client.get()
                .uri("/healthz")
                .retrieve()
                .bodyToMono(String.class)
                .block();
    }

    public String createInstance(String name) {
        return client.post()
                .uri("/instance/create")
                .bodyValue(Map.of("name", name, "auto_connect", false))
                .retrieve()
                .bodyToMono(String.class)
                .block();
    }
}
```

Configuração recomendada:

- `CHATWARP_BASE_URL=http://localhost:8080` (host)
- `CHATWARP_BASE_URL=http://chatwarp-api:8080` (container na mesma rede)

## 6. Endpoints úteis

- `GET /healthz`
- `GET /readyz`
- `GET /metrics`
- `GET /swagger`
- `GET /openapi.json`

## 7. Solução de problemas

- `Connection refused`: verifique se a API está de pé com `docker compose ps`.
- Container Java não resolve `chatwarp-api`: garanta que ambos estão na rede `chatwarp-net`.
- Mudou algo no código da API e não refletiu: rebuild com `docker compose up -d --build`.
