# Documentação de Rotas (APIs)

Abaixo está o mapeamento atual de todas as rotas da API, refletindo o `src/server/routes/mod.rs`.
As rotas estão divididas por módulos e marcadas com seu status de implementação atual no código (✅ para rotas que possuem "handlers" reais e ❌ para as que ainda retornam `501 Not Implemented`).

## Sessions

- ✅ `GET /sessions`
- ✅ `POST /sessions`
- ✅ `GET /sessions/:session`
- ❌ `PUT /sessions/:session`
- ✅ `DELETE /sessions/:session`
- ❌ `GET /sessions/:session/me`
- ✅ `POST /sessions/:session/start`
- ✅ `POST /sessions/:session/stop`
- ❌ `POST /sessions/:session/logout`
- ❌ `POST /sessions/:session/restart`
- ❌ `POST /sessions/start`
- ❌ `POST /sessions/stop`
- ❌ `POST /sessions/logout`

## Pairing

- ✅ `GET /:session/auth/qr`
- ✅ `POST /:session/auth/request-code`
- ❌ `GET /screenshot`

## Profile

- ✅ `GET /:session/profile`
- ✅ `PUT /:session/profile/name`
- ✅ `PUT /:session/profile/status`
- ✅ `PUT /:session/profile/picture`
- ❌ `DELETE /:session/profile/picture`

## Chatting

- ✅ `POST /sendText`
- ❌ `GET /sendText`
- ✅ `POST /sendImage`
- ✅ `POST /sendFile`
- ✅ `POST /sendVoice`
- ✅ `POST /sendVideo`
- ✅ `POST /send/link-custom-preview`
- ✅ `POST /sendButtons`
- ✅ `POST /sendList`
- ✅ `POST /forwardMessage`
- ✅ `POST /sendSeen`
- ✅ `POST /startTyping`
- ✅ `POST /stopTyping`
- ✅ `PUT /reaction`
- ✅ `PUT /star`
- ✅ `POST /sendPoll`
- ✅ `POST /sendPollVote`
- ✅ `POST /sendLocation`
- ✅ `POST /sendContactVcard`
- ❌ `POST /send/buttons/reply`
- ✅ `GET /messages`
- ❌ `GET /checkNumberStatus`
- ✅ `POST /reply`
- ❌ `POST /sendLinkPreview`

## Presence

- ✅ `POST /:session/presence`
- ❌ `GET /:session/presence`
- ✅ `GET /:session/presence/:chatId`
- ✅ `POST /:session/presence/:chatId/subscribe`

## Channels

- ✅ `GET /:session/channels`
- ❌ `POST /:session/channels`
- ❌ `GET /:session/channels/:id`
- ❌ `DELETE /:session/channels/:id`
- ❌ `GET /:session/channels/:id/messages/preview`
- ✅ `POST /:session/channels/:id/follow`
- ❌ `POST /:session/channels/:id/unfollow`
- ❌ `POST /:session/channels/:id/mute`
- ❌ `POST /:session/channels/:id/unmute`
- ❌ `POST /:session/channels/search/by-view`
- ✅ `POST /:session/channels/search/by-text`
- ❌ `GET /:session/channels/search/views`
- ❌ `GET /:session/channels/search/countries`
- ❌ `GET /:session/channels/search/categories`

## Status

- ✅ `POST /:session/status/text`
- ✅ `POST /:session/status/image`
- ❌ `POST /:session/status/voice`
- ✅ `POST /:session/status/video`
- ✅ `POST /:session/status/delete`
- ❌ `GET /:session/status/new-message-id`

## Chats

- ✅ `GET /:session/chats`
- ✅ `GET /:session/chats/overview`
- ❌ `POST /:session/chats/overview`
- ❌ `DELETE /:session/chats/:chatId`
- ❌ `GET /:session/chats/:chatId/picture`
- ✅ `GET /:session/chats/:chatId/messages`
- ❌ `DELETE /:session/chats/:chatId/messages`
- ✅ `POST /:session/chats/:chatId/messages/read`
- ❌ `GET /:session/chats/:chatId/messages/:messageId`
- ❌ `DELETE /:session/chats/:chatId/messages/:messageId`
- ❌ `PUT /:session/chats/:chatId/messages/:messageId`
- ❌ `POST /:session/chats/:chatId/messages/:messageId/pin`
- ❌ `POST /:session/chats/:chatId/messages/:messageId/unpin`
- ❌ `POST /:session/chats/:chatId/archive`
- ❌ `POST /:session/chats/:chatId/unarchive`
- ❌ `POST /:session/chats/:chatId/unread`

## Api Keys

- ✅ `POST /keys`
- ✅ `GET /keys`
- ❌ `PUT /keys/:id`
- ✅ `DELETE /keys/:id`

## Contacts

- ✅ `GET /contacts/all`
- ✅ `GET /contacts`
- ✅ `GET /contacts/check-exists`
- ❌ `GET /contacts/about`
- ✅ `GET /contacts/profile-picture`
- ❌ `POST /contacts/block`
- ❌ `POST /contacts/unblock`
- ❌ `PUT /:session/contacts/:chatId`
- ❌ `GET /:session/lids`
- ❌ `GET /:session/lids/count`
- ❌ `GET /:session/lids/:lid`
- ❌ `GET /:session/lids/pn/:phoneNumber`

## Groups

- ✅ `POST /:session/groups`
- ✅ `GET /:session/groups`
- ❌ `GET /:session/groups/join-info`
- ✅ `POST /:session/groups/join`
- ❌ `GET /:session/groups/count`
- ❌ `POST /:session/groups/refresh`
- ✅ `GET /:session/groups/:id`
- ❌ `DELETE /:session/groups/:id`
- ✅ `POST /:session/groups/:id/leave`
- ❌ `GET /:session/groups/:id/picture`
- ❌ `PUT /:session/groups/:id/picture`
- ❌ `DELETE /:session/groups/:id/picture`
- ❌ `PUT /:session/groups/:id/description`
- ❌ `PUT /:session/groups/:id/subject`
- ❌ `PUT /:session/groups/:id/settings/security/info-admin-only`
- ❌ `GET /:session/groups/:id/settings/security/info-admin-only`
- ❌ `PUT /:session/groups/:id/settings/security/messages-admin-only`
- ❌ `GET /:session/groups/:id/settings/security/messages-admin-only`
- ✅ `GET /:session/groups/:id/invite-code`
- ❌ `POST /:session/groups/:id/invite-code/revoke`
- ✅ `GET /:session/groups/:id/participants`
- ❌ `GET /:session/groups/:id/participants/v2`
- ✅ `POST /:session/groups/:id/participants/add`
- ✅ `POST /:session/groups/:id/participants/remove`
- ❌ `POST /:session/groups/:id/admin/promote`
- ❌ `POST /:session/groups/:id/admin/demote`

## Calls

- ✅ `POST /:session/calls/reject`

## Events

- ✅ `POST /:session/events`

## Labels

- ✅ `GET /:session/labels`
- ✅ `POST /:session/labels`
- ❌ `PUT /:session/labels/:labelId`
- ❌ `DELETE /:session/labels/:labelId`
- ❌ `GET /:session/labels/chats/:chatId`
- ✅ `PUT /:session/labels/chats/:chatId`
- ✅ `GET /:session/labels/:labelId/chats`

## Media

- ✅ `POST /:session/media/convert/voice`
- ✅ `POST /:session/media/convert/video`

## Apps

- ✅ `GET /apps`
- ✅ `POST /apps`
- ❌ `GET /apps/:id`
- ❌ `PUT /apps/:id`
- ❌ `DELETE /apps/:id`
- ❌ `GET /apps/chatwoot/locales`

## Observability

- ✅ `GET /ping`
- ✅ `GET /health`
- ❌ `GET /server/version`
- ❌ `GET /server/environment`
- ✅ `GET /server/status`
- ❌ `POST /server/stop`
- ❌ `GET /server/debug/cpu`
- ❌ `GET /server/debug/heapsnapshot`
- ❌ `GET /server/debug/browser/trace/:session`
- ❌ `GET /version`

## Implementação Genérica Padrão

As rotas não implementadas (marcadas com ❌) retornam `501 Not Implemented` via o fallback:

```json
{
  "error": "not_implemented",
  "route": "<path atual da rota>"
}
```
