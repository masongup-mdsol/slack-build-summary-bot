use super::*;
use serde_json::json;

#[test]
fn handle_message_object() {
    let event = json!({
        "type": "message",
        "channel": "C024BE91L",
        "user": "U2147483697",
        "text": "Live long and prospect.",
        "ts": "1355517523.000005",
        "event_ts": "1355517523.000005",
        "channel_type": "channel",
        "attachments": { "one": "item", "two": "items" }
    });
    assert!(handle_event_object(&event.as_object().unwrap()).is_ok());
}
