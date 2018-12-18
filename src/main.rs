#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate log;

use std::env;
use rocket::*;
use rocket::http::*;
use serde_json::{Value, json};
use serde_derive::Deserialize;
use rocket_contrib::json::Json;
use regex;

#[cfg(test)]
mod test;

#[post("/event", data = "<message>")]
fn message_receive(message: Json<Value>, slack_params: State<SlackParams>) -> Result<Json<Value>, Status> {
    match message.into_inner() {
        Value::Object(message_map) => {
            let token_maybe = message_map.get("token").and_then(|token_val| token_val.as_str());
            if token_maybe.is_none() || token_maybe.unwrap() != slack_params.verification_token {
                info!("Got a bad or empty verification token");
                return Err(Status::BadRequest);
            }
            match message_map.get("type").and_then(|type_val| type_val.as_str()) {
                Some("url_verification") => message_map.get("challenge")
                    .and_then(|challenge_val| challenge_val.as_str())
                    .and_then(|challenge_str| Some(Ok(Json(json!({"challenge": challenge_str})))))
                    .unwrap_or_else(|| Err(Status::BadRequest)),
                Some("event_callback") => {
                    match message_map.get("event") {
                        Some(Value::Object(event_obj)) => handle_event_object(event_obj).map_err(|e| {
                            info!("{}", e);
                            Status::BadRequest
                        }),
                        _ => {
                            info!("Got an event_callback without an event");
                            Err(Status::BadRequest)
                        },
                    }
                },
                _ => {
                    info!("Got invalid request, full body '{}'", serde_json::to_string(&message_map).unwrap());
                    Err(Status::BadRequest)
                },
            }
        }
        message_other => {
            info!("Got invalid request, full body '{}'", serde_json::to_string(&message_other).unwrap());
            Err(Status::BadRequest)
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
    color: String,
    id: u32,
    title: Option<String>,
    text: String,
    fallback: String,
}

fn handle_event_object(event: &serde_json::map::Map<String, Value>) -> Result<Json<Value>, String> {
    match event.get("type").and_then(|t| t.as_str()) {
        Some("message") => {
            match serde_json::from_value::<Message>(Value::Object(event.clone())) {
                Err(err) => Err(format!("Failed to parse message into expected struct: {}", err)),
                Ok(message) => {
                    if let Some(bot_id) = message.bot_id {
                        if bot_id == "B2NJE012S" {
                            info!("Got message from GoCD Bot");
                            if let Some(attachments) = message.attachments {
                                if let Some(first_attachment) = attachments.first() {
                                    info!("Got attachments with title {:?} and text {}",
                                          first_attachment.title, first_attachment.text);
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

#[get("/app_status")]
fn app_status() -> Status {
    Status::Ok
}

#[allow(dead_code)]
struct SlackParams {
    verification_token: String,
    app_id: String,
    client_id: String,
    client_secret: String,
    signing_secret: String,
}

impl SlackParams {
    fn from_env(is_prod: bool) -> SlackParams {
        fn get_env_var(name: &str) -> String {
            env::var(name).expect(&format!("Unable to access env var {}", name))
        }
        if is_prod {
            SlackParams {
                verification_token: get_env_var("SLACK_VERIFICATION_TOKEN"),
                app_id: get_env_var("SLACK_APP_ID"),
                client_id: get_env_var("SLACK_CLIENT_ID"),
                client_secret: get_env_var("SLACK_CLIENT_SECRET"),
                signing_secret: get_env_var("SLACK_SIGNING_SECRET"),
            }
        }
        else {
            SlackParams {
                verification_token: "test".to_string(),
                app_id: "test".to_string(),
                client_id: "test".to_string(),
                client_secret: "test".to_string(),
                signing_secret: "test".to_string(),
            }
        }
    }
}

fn init_logging() {
    let file_appender = log4rs::append::file::FileAppender::builder()
        .build("log/production.log")
        .unwrap();
    let config = log4rs::config::Config::builder()
        .appender(log4rs::config::Appender::builder().build("file", Box::new(file_appender)))
        .build(log4rs::config::Root::builder().appender("file").build(log::LevelFilter::Info)).unwrap();
    log4rs::init_config(config).expect("Tried to init logging with logger already set");
}

fn main() {
    #[cfg(not(debug_assertions))]
    init_logging();
    let app = rocket::ignite();
    let is_prod = app.config().environment.is_prod();
    app
        .mount("/", routes![message_receive, app_status])
        .manage(SlackParams::from_env(is_prod))
        .launch();
}
