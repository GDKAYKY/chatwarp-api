use axum::{
    Json,
    Router,
    extract::{OriginalUri, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde_json::json;

mod apps;
mod calls;
mod channels;
mod chats;
mod chatting;
mod contacts;
mod events;
mod groups;
mod helpers;
mod keys;
mod labels;
mod media;
mod observability;
mod pairing;
mod presence;
mod profile;
mod sessions;
mod status;

use std::sync::Arc;
use crate::server::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::<Arc<AppState>>::new()
        // Sessions
        .route("/sessions", get(sessions::list_sessions).post(sessions::create_session))
        .route(
            "/sessions/:session",
            get(sessions::get_session)
                .put(not_implemented)
                .delete(sessions::delete_session),
        )
        .route("/sessions/:session/me", get(not_implemented))
        .route("/sessions/:session/start", post(sessions::start_session))
        .route("/sessions/:session/stop", post(sessions::stop_session))
        .route("/sessions/:session/logout", post(not_implemented))
        .route("/sessions/:session/restart", post(not_implemented))
        .route("/sessions/start", post(not_implemented))
        .route("/sessions/stop", post(not_implemented))
        .route("/sessions/logout", post(not_implemented))
        // Pairing
        .route("/:session/auth/qr", get(pairing::get_qr))
        .route("/:session/auth/request-code", post(pairing::request_code))
        .route("/screenshot", get(not_implemented))
        // Profile
        .route("/:session/profile", get(profile::get_profile))
        .route("/:session/profile/name", put(profile::update_name))
        .route("/:session/profile/status", put(profile::update_status))
        .route(
            "/:session/profile/picture",
            put(profile::update_picture).delete(not_implemented),
        )
        // Chatting
        .route("/sendText", post(chatting::send_text).get(not_implemented))
        .route("/sendImage", post(chatting::send_image))
        .route("/sendFile", post(chatting::send_file))
        .route("/sendVoice", post(chatting::send_voice))
        .route("/sendVideo", post(chatting::send_video))
        .route("/send/link-custom-preview", post(chatting::send_link_custom_preview))
        .route("/sendButtons", post(chatting::send_buttons))
        .route("/sendList", post(chatting::send_list))
        .route("/forwardMessage", post(chatting::forward_message))
        .route("/sendSeen", post(chatting::send_seen))
        .route("/startTyping", post(chatting::start_typing))
        .route("/stopTyping", post(chatting::stop_typing))
        .route("/reaction", put(chatting::reaction))
        .route("/star", put(chatting::star))
        .route("/sendPoll", post(chatting::send_poll))
        .route("/sendPollVote", post(chatting::send_poll_vote))
        .route("/sendLocation", post(chatting::send_location))
        .route("/sendContactVcard", post(chatting::send_contact_vcard))
        .route("/send/buttons/reply", post(not_implemented))
        .route("/messages", get(chatting::list_messages_handler))
        .route("/checkNumberStatus", get(not_implemented))
        .route("/reply", post(chatting::reply_message))
        .route("/sendLinkPreview", post(not_implemented))
        // Presence
        .route("/:session/presence", post(presence::set_presence).get(not_implemented))
        .route("/:session/presence/:chatId", get(presence::get_presence))
        .route(
            "/:session/presence/:chatId/subscribe",
            post(presence::subscribe),
        )
        // Channels
        .route("/:session/channels", get(channels::list_channels).post(not_implemented))
        .route("/:session/channels/:id", get(not_implemented).delete(not_implemented))
        .route(
            "/:session/channels/:id/messages/preview",
            get(not_implemented),
        )
        .route("/:session/channels/:id/follow", post(channels::follow_channel))
        .route("/:session/channels/:id/unfollow", post(not_implemented))
        .route("/:session/channels/:id/mute", post(not_implemented))
        .route("/:session/channels/:id/unmute", post(not_implemented))
        .route("/:session/channels/search/by-view", post(not_implemented))
        .route("/:session/channels/search/by-text", post(channels::search_by_text))
        .route("/:session/channels/search/views", get(not_implemented))
        .route("/:session/channels/search/countries", get(not_implemented))
        .route("/:session/channels/search/categories", get(not_implemented))
        // Status
        .route("/:session/status/text", post(status::status_text))
        .route("/:session/status/image", post(status::status_image))
        .route("/:session/status/voice", post(not_implemented))
        .route("/:session/status/video", post(status::status_video))
        .route("/:session/status/delete", post(status::status_delete))
        .route("/:session/status/new-message-id", get(not_implemented))
        // Chats
        .route("/:session/chats", get(chats::list_chats))
        .route("/:session/chats/overview", get(chats::overview).post(not_implemented))
        .route("/:session/chats/:chatId", delete(not_implemented))
        .route("/:session/chats/:chatId/picture", get(not_implemented))
        .route(
            "/:session/chats/:chatId/messages",
            get(chats::messages).delete(not_implemented),
        )
        .route("/:session/chats/:chatId/messages/read", post(chats::read_messages))
        .route(
            "/:session/chats/:chatId/messages/:messageId",
            get(not_implemented).delete(not_implemented).put(not_implemented),
        )
        .route(
            "/:session/chats/:chatId/messages/:messageId/pin",
            post(not_implemented),
        )
        .route(
            "/:session/chats/:chatId/messages/:messageId/unpin",
            post(not_implemented),
        )
        .route("/:session/chats/:chatId/archive", post(not_implemented))
        .route("/:session/chats/:chatId/unarchive", post(not_implemented))
        .route("/:session/chats/:chatId/unread", post(not_implemented))
        // Api Keys
        .route("/keys", post(keys::create_key).get(keys::list_keys))
        .route("/keys/:id", put(not_implemented).delete(keys::revoke_key))
        // Contacts
        .route("/contacts/all", get(contacts::list_contacts_all))
        .route("/contacts", get(contacts::list_contacts))
        .route("/contacts/check-exists", get(contacts::check_exists))
        .route("/contacts/about", get(not_implemented))
        .route("/contacts/profile-picture", get(contacts::profile_picture))
        .route("/contacts/block", post(not_implemented))
        .route("/contacts/unblock", post(not_implemented))
        .route("/:session/contacts/:chatId", put(not_implemented))
        .route("/:session/lids", get(not_implemented))
        .route("/:session/lids/count", get(not_implemented))
        .route("/:session/lids/:lid", get(not_implemented))
        .route("/:session/lids/pn/:phoneNumber", get(not_implemented))
        // Groups
        .route("/:session/groups", post(groups::create_group).get(groups::list_groups))
        .route("/:session/groups/join-info", get(not_implemented))
        .route("/:session/groups/join", post(groups::join_group))
        .route("/:session/groups/count", get(not_implemented))
        .route("/:session/groups/refresh", post(not_implemented))
        .route(
            "/:session/groups/:id",
            get(groups::get_group).delete(not_implemented),
        )
        .route("/:session/groups/:id/leave", post(groups::leave_group))
        .route(
            "/:session/groups/:id/picture",
            get(not_implemented).put(not_implemented).delete(not_implemented),
        )
        .route("/:session/groups/:id/description", put(not_implemented))
        .route("/:session/groups/:id/subject", put(not_implemented))
        .route(
            "/:session/groups/:id/settings/security/info-admin-only",
            put(not_implemented).get(not_implemented),
        )
        .route(
            "/:session/groups/:id/settings/security/messages-admin-only",
            put(not_implemented).get(not_implemented),
        )
        .route("/:session/groups/:id/invite-code", get(groups::invite_code))
        .route("/:session/groups/:id/invite-code/revoke", post(not_implemented))
        .route("/:session/groups/:id/participants", get(groups::participants))
        .route("/:session/groups/:id/participants/v2", get(not_implemented))
        .route("/:session/groups/:id/participants/add", post(groups::add_participants))
        .route(
            "/:session/groups/:id/participants/remove",
            post(groups::remove_participants),
        )
        .route("/:session/groups/:id/admin/promote", post(not_implemented))
        .route("/:session/groups/:id/admin/demote", post(not_implemented))
        // Calls
        .route("/:session/calls/reject", post(calls::reject_call))
        // Events
        .route("/:session/events", post(events::post_event))
        // Labels
        .route("/:session/labels", get(labels::list_labels).post(labels::create_label))
        .route("/:session/labels/:labelId", put(not_implemented).delete(not_implemented))
        .route(
            "/:session/labels/chats/:chatId",
            get(not_implemented).put(labels::apply_label),
        )
        .route("/:session/labels/:labelId/chats", get(labels::chats_by_label))
        // Media
        .route("/:session/media/convert/voice", post(media::convert_voice))
        .route("/:session/media/convert/video", post(media::convert_video))
        // Apps
        .route("/apps", get(apps::list_apps).post(apps::create_app))
        .route("/apps/:id", get(not_implemented).put(not_implemented).delete(not_implemented))
        .route("/apps/chatwoot/locales", get(not_implemented))
        // Observability
        .route("/ping", get(observability::ping))
        .route("/health", get(observability::health))
        .route("/server/version", get(not_implemented))
        .route("/server/environment", get(not_implemented))
        .route("/server/status", get(observability::server_status))
        .route("/server/stop", post(not_implemented))
        .route("/server/debug/cpu", get(not_implemented))
        .route("/server/debug/heapsnapshot", get(not_implemented))
        .route("/server/debug/browser/trace/:session", get(not_implemented))
        .route("/version", get(not_implemented))
}

async fn not_implemented(
    State(_state): State<Arc<AppState>>,
    uri: OriginalUri,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "not_implemented",
            "route": uri.0.path(),
        })),
    )
}
