Excelente crítica. Agora você falou como dono do projeto, não como consumidor de resposta bonita.

Você está 100% certo em vários pontos:

* Prioridade sumir é regressão real.
* “Todas pendentes” é inútil num doc operacional.
* Alertas de Axum e health checks têm valor técnico concreto.
* Call-to-action final é ruído em documentação de repositório.

Você não quer README marketing.
Você quer **documento de execução**.

Então vamos refazer direito.

Sem emoji.
Sem fluff.
Com prioridade.
Com rotas explícitas.
Com observações que evitam bug silencioso.

---

# ROUTES.md

> Documentação de rotas da API.
> Status, prioridade de implementação e observações técnicas.

---

# Status Geral

| Grupo                  | Status      | Prioridade |
| ---------------------- | ----------- | ---------- |
| Core                   | ✅ Funcional | —          |
| Instance               | ✅ Funcional | —          |
| Message                | ✅ Funcional | —          |
| Call                   | ✅ Funcional | —          |
| Channel / Event        | ⚠️ Parcial  | Média      |
| Chat                   | ❌ 501       | Alta       |
| Group                  | ❌ 501       | Média      |
| Settings               | ❌ 501       | Média      |
| Business               | ❌ 501       | Baixa      |
| Template               | ❌ 501       | Baixa      |
| Proxy                  | ❌ 501       | Segurar    |
| Label                  | ❌ 501       | Segurar    |
| Chatbot Providers      | ❌ 501       | Faseado    |
| S3                     | ❌ 501       | Segurar    |
| Event Providers extras | ❌ 501       | Fora R1    |

---

# Implementadas (funcionais)

## Core

```
GET  /
POST /verify-creds
GET  /metrics
GET  /ws
GET  /manager
GET  /manager/{*path}
GET  /assets/*file
```

Pendências críticas:

```
GET /healthz   ← liveness
GET /readyz    ← readiness
```

`/metrics` não substitui health checks.

---

## Instance `/instance`

```
POST   /create
POST   /restart/:instance_name
GET    /connect/:instance_name
GET    /connectionState/:instance_name
GET    /fetchInstances
POST   /setPresence/:instance_name
DELETE /logout/:instance_name
DELETE /delete/:instance_name
```

Observação: `instance_name` representa sessão ativa WhatsApp.

---

## Message `/message`

```
POST /:operation/:instance_name
```

Operações permitidas:

* sendTemplate
* sendText
* sendMedia
* sendPtv
* sendWhatsAppAudio
* sendStatus
* sendSticker
* sendLocation
* sendContact
* sendReaction
* sendPoll
* sendList
* sendButtons

Observação técnica:
Handler genérico facilita extensão, mas reduz descritibilidade OpenAPI.
Avaliar documentação explícita por operação no futuro.

---

## Call `/call`

```
POST /offer/:instance_name
```

---

## Channel / Event (parcial)

### Webhook

```
POST /webhook/evolution
GET  /webhook/meta
POST /webhook/meta
POST /webhook/set/:instance_name
GET  /webhook/find/:instance_name
```

### WebSocket

```
POST /websocket/set/:instance_name
GET  /websocket/find/:instance_name
```

### RabbitMQ

```
POST /rabbitmq/set/:instance_name
GET  /rabbitmq/find/:instance_name
```

### Proxy Baileys

```
POST /baileys/:operation/:instance_name
```

Providers ainda não implementados:

* nats
* sqs
* pusher
* kafka

Fora do escopo R1.

---

# 501 — Prioridade Alta

## Chat `/chat/*`

```
GET  /findMessages/:instance_name
GET  /findChats/:instance_name
GET  /findContacts/:instance_name
POST /markMessageAsRead/:instance_name
POST /archiveChat/:instance_name
POST /deleteMessage/:instance_name
GET  /fetchProfilePicture/:instance_name
POST /updateMessage/:instance_name
```

Sem essas rotas, integrações dependem exclusivamente de webhook.

---

# 501 — Prioridade Média

## Group `/group/*`

```
POST /create/:instance_name
GET  /findGroupInfos/:instance_name
GET  /fetchAllGroups/:instance_name
GET  /listInvite/:instance_name
POST /updateParticipant/:instance_name
POST /updateSetting/:instance_name
POST /updateGroupPicture/:instance_name
POST /updateGroupSubject/:instance_name
POST /updateGroupDescription/:instance_name
POST /leaveGroup/:instance_name
```

---

## Settings `/settings/*`

```
POST /set/:instance_name
GET  /find/:instance_name
```

---

# 501 — Prioridade Baixa

## Business `/business/*`

```
GET /fetchCatalog/:instance_name
GET /fetchCollections/:instance_name
GET /fetchProducts/:instance_name
```

## Template `/template/*`

```
POST   /create/:instance_name
GET    /find/:instance_name
POST   /update/:instance_name
DELETE /delete/:instance_name
```

---

# 501 — Segurar

## Proxy `/proxy/*`

```
POST /set/:instance_name
GET  /find/:instance_name
```

## Label `/label/*`

```
POST /handle/:instance_name
GET  /find/:instance_name
```

## S3 `/s3/*`

```
POST /set/:instance_name
GET  /find/:instance_name
```

---

# Chatbot Providers (faseado)

Webhook-based (implementar primeiro):

* evolutionBot
* chatwoot
* n8n
* typebot

API-based (fase posterior):

* openai
* dify
* flowise
* evoai

Implementação parcial é pior que 501.

---

# Observações Técnicas Críticas

## Autenticação

Guard de `apikey` aplicado nas rotas protegidas.
Rotas públicas: `/`, `/metrics`, `/ws`.

Avaliar escopo por instância no futuro.

---

## Sintaxe Axum

Usar sempre `:param`.

Nunca `{param}`.

Axum não falha em compile-time com sintaxe incorreta.
A rota simplesmente não casa silenciosamente.

Recomendado: teste de integração validando matching das rotas principais.

---

## Health Checks

```
GET /healthz
GET /readyz
```

Obrigatórios antes de deploy em qualquer orquestrador (k8s, fly.io, railway, etc.).

`readyz` deve validar:

* conexão ativa da instância
* dependências externas (se aplicável)
* sidecar disponível

---