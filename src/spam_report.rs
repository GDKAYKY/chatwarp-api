use crate::client::Client;
use crate::request::{InfoQuery, IqError};
use warp_core_binary::jid::{Jid, SERVER_JID};
use warp_core_binary::node::NodeContent;

pub use warp_core::types::{SpamFlow, SpamReportRequest, SpamReportResult, build_spam_list_node};

impl Client {
    /// Send a spam report to WhatsApp.
    ///
    /// This sends a `spam_list` IQ stanza to report one or more messages as spam.
    ///
    /// # Arguments
    /// * `request` - The spam report request containing message details
    ///
    /// # Returns
    /// * `Ok(SpamReportResult)` - If the report was successfully submitted
    /// * `Err` - If there was an error sending or processing the report
    ///
    /// # Example
    /// ```rust,ignore
    /// let result = client.send_spam_report(SpamReportRequest {
    ///     message_id: "MSG_ID".to_string(),
    ///     message_timestamp: 1234567890,
    ///     from_jid: Some(sender_jid),
    ///     spam_flow: SpamFlow::MessageMenu,
    ///     ..Default::default()
    /// }).await?;
    /// ```
    pub async fn send_spam_report(
        &self,
        request: SpamReportRequest,
    ) -> Result<SpamReportResult, IqError> {
        let spam_list_node = build_spam_list_node(&request);

        let server_jid = Jid::new("", SERVER_JID);

        let query = InfoQuery::set(
            "spam",
            server_jid,
            Some(NodeContent::Nodes(vec![spam_list_node])),
        );

        let response = self.send_iq(query).await?;

        // Extract report_id from response if present
        let report_id = response
            .get_optional_child_by_tag(&["report_id"])
            .and_then(|n| match &n.content {
                Some(NodeContent::String(s)) => Some(s.clone()),
                _ => None,
            });

        Ok(SpamReportResult { report_id })
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/spam_report_tests.rs"));
}
