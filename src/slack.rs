use std::io::Read;

use serde_json::{Value, Map};
use serde_derive::Deserialize;

use rocket_contrib::json::Json;
use rocket::request::Request;
use rocket::outcome::Outcome::*;
use rocket::data::{self, FromDataSimple};
use rocket::Data;
use rocket::http::Status;

use ring::hmac::{verify, VerificationKey};
use regex::Regex;
use hex;
use chrono::prelude::*;
use time::Duration;

use crate::build_info_manager::AcceptBuildInfo;

#[allow(dead_code)]
pub struct SlackParams {
    pub verification_token: String,
    pub app_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub signing_secret: VerificationKey,
    pub gocd_bod_id: String,
    pub instance_token: String,
    pub title_match_regex: Regex,
}

pub struct VerifiedSlackJson {
    json_obj: Map<String, Value>,
}

impl VerifiedSlackJson {
    pub fn json_obj(&self) -> &Map<String, Value> {
        &self.json_obj
    }
}

const LIMIT: u64 = 4000;

impl FromDataSimple for VerifiedSlackJson {
    type Error = String;

    fn from_data(request: &Request, data: Data) -> data::Outcome<Self, Self::Error> {
        let header_map = request.headers();
        let maybe_sig = header_map.get_one("X-Slack-Signature")
            .and_then(|raw| raw.split('=').nth(1))
            .and_then(|hex| hex::decode(hex).ok());
        let maybe_ts = header_map.get_one("X-Slack-Request-Timestamp")
            .and_then(|raw| raw.parse().ok());
        if maybe_sig.is_none() || maybe_ts.is_none() {
            return Failure((Status::Unauthorized , "Missing Signature Headers!".to_string()));
        }

        let timestamp_diff = Utc.timestamp(maybe_ts.unwrap(), 0) - Utc::now();
        if timestamp_diff > Duration::seconds(60) || timestamp_diff < Duration::seconds(-60) {
            return Failure((Status::Unauthorized , "Timestamp out of range".to_string()));
        }

        let mut raw_request = String::new();
        if let Err(e) = data.open().take(LIMIT).read_to_string(&mut raw_request) {
            return Failure((Status::InternalServerError , format!("Some kind of badness: {}", e)));
        }

        let string_to_sign = format!("v0:{}:{}", &maybe_ts.unwrap(), &raw_request);

        //allow unwrap here because if there isn't a SlackParams state then something is fundamentally wrong and
        //we should blow up
        let verify_key = &request.guard::<rocket::State<SlackParams>>().unwrap().signing_secret;
        let signature = maybe_sig.unwrap();
        if let Err(_) = verify(&verify_key, string_to_sign.as_bytes(), &signature) {
            return Failure((Status::Unauthorized , "Failed to verify signature".to_string()));
        }

        match serde_json::from_str(&raw_request) {
            Ok(Value::Object(json)) => Success(VerifiedSlackJson { json_obj: json }),
            _ => Failure((Status::BadRequest, format!("Unable to parse JSON"))),
        }
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Message {
    channel_type: String,
    channel: String,
    user: Option<String>,
    text: Option<String>,
    ts: String,
    event_ts: Option<String>,
    subtype: Option<String>,
    bot_id: Option<String>,
    attachments: Option<Vec<Attachment>>,
    client_msg_id: Option<String>,
    parent_user_id: Option<String>,
    previous_message: Option<Value>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Attachment {
    color: Option<String>,
    id: Option<u32>,
    title: Option<String>,
    text: Option<String>,
    fallback: Option<String>,
}

pub fn handle_event_object(event: &serde_json::map::Map<String, Value>, params: &SlackParams, collector: &AcceptBuildInfo) -> Result<Json<Value>, String> {
    match event.get("type").and_then(|t| t.as_str()) {
        Some("message") => {
            match serde_json::from_value::<Message>(Value::Object(event.clone())) {
                Err(err) => Err(format!("Failed to parse message into expected struct: {}", err)),
                Ok(message) => {
                    if message.bot_id.is_some() && message.bot_id.unwrap() == params.gocd_bod_id {
                        if let Some(attachments) = message.attachments {
                            if let Some(first_attachment) = attachments.first() {
                                info!("Got attachments with title {:?} and text {:?}",
                                    first_attachment.title, first_attachment.text);
                                if let Some(title) = &first_attachment.title {
                                    process_message(&title, &params, collector);
                                }
                            }
                        }
                    }
                    Ok(Json(Value::Null))
                },
            }
        },
        Some(type_str) => Err(format!("Got unknown event type {}", type_str)),
        None => Err("Got no event type".to_string()),
    }
}

pub fn get_regex_string() -> String {
    r"^Go pipeline stage \[(?P<stage_name>[\w_]+)/(?P<build_num>\d+)/(?P<step_name>\w+)/\d+\] (?P<pass_fail>passed|failed)".to_string()
}

fn process_message(message_text: &String, params: &SlackParams, collector: &AcceptBuildInfo) {
    match params.title_match_regex.captures(message_text) {
        None => info!("Unable to handle message {} with regex", message_text),
        Some(captures) => {
            let stage_name = captures.name("stage_name");
            let build_num = captures.name("build_num").and_then(|m| m.as_str().parse().ok());
            let step_name = captures.name("step_name");
            let pass_fail = captures.name("pass_fail");
            if stage_name.is_some() && build_num.is_some() && step_name.is_some() && pass_fail.is_some() {
                collector.new_build_message(
                    stage_name.unwrap().as_str(),
                    build_num.unwrap(),
                    step_name.unwrap().as_str(),
                    pass_fail.unwrap().as_str()
                );
            }
        }
    }
}
