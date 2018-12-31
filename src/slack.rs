use serde_json::{Value};
use serde_derive::Deserialize;
use regex::Regex;
use rocket_contrib::json::Json;
use rocket::request::{self, FromRequest, Request};
use rocket::outcome::Outcome::*;
use ring::hmac::{verify, VerificationKey};

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

pub fn check_token(message_map: &serde_json::map::Map<String, Value>, params: &SlackParams) -> Result<(), String> {
    let token_maybe = message_map.get("token").and_then(|token_val| token_val.as_str());
    if token_maybe.is_none() || token_maybe.unwrap() != params.verification_token {
        Err("Got a bad or empty verification token".to_string())
    }
    else {
        Ok(())
    }
}

pub struct AuthHeaders {
    signature: String,
    timestamp_str: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for AuthHeaders {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let header_map = request.headers();
        let maybe_sig = header_map.get_one("X-Slack-Signature");
        let maybe_ts = header_map.get_one("X-Slack-Request-Timestamp");
        if maybe_sig.is_some() && maybe_ts.is_some() {
            Success(AuthHeaders {
                signature: maybe_sig.unwrap().to_string(),
                timestamp_str: maybe_ts.unwrap().to_string(),
            })
        }
        else {
            Failure((rocket::http::Status::Unauthorized , "Missing Signature Headers!".to_string()))
        }
    }
}

impl AuthHeaders {
    pub fn validate_with_body(&self, body: &str, verify_key: &VerificationKey) -> Result<(), String> {
        let string_to_sign = format!("v0:{}:{}", &self.timestamp_str, &body);
        let signature = self.signature.split('=').nth(1).ok_or_else(|| "bad signature".to_string())?;
        let result = verify(&verify_key, string_to_sign.as_bytes(), signature.as_bytes())
            .map_err(|_| "bad signature".to_string());
        if result.is_err() {
            info!("Failed to verify signature. Using string to sign '{}' and signature str '{}'",
                   &string_to_sign, &signature);
        }
        result
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
