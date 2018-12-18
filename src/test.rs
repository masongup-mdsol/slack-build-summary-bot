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
        "channel_type": "channel"
    });
    let result = handle_event_object(&event.as_object().unwrap());
    assert!(result.is_ok(), "Error is: {:?}", result.err().unwrap());
}

#[test]
fn handle_message_object_with_attachment() {
    let event = json!({
        "type": "message",
        "subtype": "bot_message",
        "channel": "C024BE91L",
        "text": "",
        "bot_id": "bot",
        "ts": "1355517523.000005",
        "event_ts": "1355517523.000005",
        "channel_type": "channel",
        "attachments": [
            {
                "color": "2eb886",
                "fallback": "fallback",
                "id": 1,
                "title": "title",
                "text": "text"
            },
            {
                "color": "2eb886",
                "fallback": "fallback",
                "id": 2,
                "text": "some text"
            }
        ]
    });
    let result = handle_event_object(&event.as_object().unwrap());
    assert!(result.is_ok(), "Error is: {:?}", result.err().unwrap());
}
