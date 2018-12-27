use serde_json::json;
use crate::slack::{SlackParams, handle_event_object};
use crate::build_info_manager::AcceptBuildInfo;
use std::cell::RefCell;

struct DummyBuildInfoAcceptor {
    builds_received: RefCell<Vec<(String, u32)>>,
}

impl DummyBuildInfoAcceptor {
    fn new() -> DummyBuildInfoAcceptor {
        DummyBuildInfoAcceptor {
            builds_received: RefCell::new(vec![]),
        }
    }
}

impl AcceptBuildInfo for DummyBuildInfoAcceptor {
    fn new_build_message(&self, stage_name: &str, build_num: u32, _build_step: &str, _pass_fail: &str) {
        self.builds_received.borrow_mut().push((stage_name.to_string(), build_num));
    }
}

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
    let build_info = DummyBuildInfoAcceptor::new();
    let dummy_params = SlackParams::from_env(false);
    let result = handle_event_object(&event.as_object().unwrap(), &dummy_params, &build_info);
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
            }
        ]
    });
    let build_info = DummyBuildInfoAcceptor::new();
    let dummy_params = SlackParams::from_env(false);
    let result = handle_event_object(&event.as_object().unwrap(), &dummy_params, &build_info);
    assert!(result.is_ok(), "Error is: {:?}", result.err().unwrap());
}

#[test]
fn handle_gocd_build_message() {
    let dummy_params = SlackParams::from_env(false);
    let event = json!({
        "type": "message",
        "bot_id": dummy_params.gocd_bod_id,
        "channel": "C024BE91L",
        "channel_type": "channel",
        "ts": "1355517523.000005",
        "attachments": [
            {
                "color": "2eb886",
                "fallback": "fallback",
                "id": 1,
                "title": "Go pipeline stage [Zeus_ECS_Distro/20/Deploy/1] passed",
                "text": "text"
            },
        ],
    });
    let build_info = DummyBuildInfoAcceptor::new();
    let result = handle_event_object(&event.as_object().unwrap(), &dummy_params, &build_info);
    assert!(result.is_ok(), "Error is: {:?}", result.err().unwrap());
    let builds_received_vec = build_info.builds_received.borrow();
    let info_result = builds_received_vec.first().expect("Did not receive an item");
    assert_eq!(info_result.0, "Zeus_ECS_Distro");
    assert_eq!(info_result.1, 20);
}
