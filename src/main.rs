#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate log;

use rocket::*;
use rocket::http::*;
use serde_json::{Value, json};
use serde_derive::Deserialize;
use rocket_contrib::json::Json;

#[cfg(test)]
mod test;

#[post("/event", data = "<message>")]
fn message_receive(message: Json<Value>, slack_params: State<SlackParams>) -> Result<Json<Value>, Status> {
    if let Value::Object(message_map) = message.into_inner() {
        let token = message_map.get("token").and_then(|token_val| token_val.as_str());
        match token {
            Some(token_str) => info!("Got request token {} against expected token {}", token_str, slack_params.verification_token),
            None => info!("Did not get request token")
        }
        match message_map.get("type").and_then(|type_val| type_val.as_str()) {
            Some("url_verification") => message_map.get("challenge")
                .and_then(|challenge_val| challenge_val.as_str())
                .and_then(|challenge_str| Some(Ok(Json(json!({"challenge": challenge_str})))))
                .unwrap_or_else(|| Err(Status::BadRequest)),
            Some("event_callback") => {
                match message_map.get("event") {
                    Some(Value::Object(event_obj)) => handle_event_object(event_obj),
                    _ => Err(Status::BadRequest),
                }
            },
            _ => Err(Status::BadRequest),
        }
    }
    else {
        Err(Status::BadRequest)
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Message {
    channel_type: String,
    channel: String,
    user: String,
    text: String,
    ts: String,
}

fn handle_event_object(event: &serde_json::map::Map<String, Value>) -> Result<Json<Value>, Status> {
    match event.get("type").and_then(|t| t.as_str()) {
        Some("message") => {
            match serde_json::from_value::<Message>(Value::Object(event.clone())) {
                Err(_) => Err(Status::BadRequest),
                Ok(message) => {
                    info!("Got message from user {} on channel {} with text '{}'", message.user, message.channel, message.text);
                    Ok(Json(Value::Null))
                },
            }
        },
        _ => Err(Status::BadRequest)
    }
}

#[get("/app_status")]
fn app_status() -> Status {
    Status::Ok
}

fn main() {
    rocket::ignite()
        .mount("/", routes![message_receive, app_status])
        .launch();
}
