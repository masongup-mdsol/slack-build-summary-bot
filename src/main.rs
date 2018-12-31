#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate log;

use std::env;
use rocket::*;
use rocket::http::*;
use serde_json::{Value, json};
use rocket_contrib::json::Json;
use regex::Regex;
use ring::digest::SHA256;
use ring::hmac::VerificationKey;


mod slack;
use crate::slack::{SlackParams, handle_event_object, check_token, get_regex_string, AuthHeaders};

mod build_info_manager;
use crate::build_info_manager::{BuildInfoManager};

#[cfg(test)]
mod test;

#[post("/event", data = "<message>")]
fn message_receive(
    message: String,
    slack_params: State<SlackParams>,
    collector: State<BuildInfoManager>,
    auth_headers: AuthHeaders
)
-> Result<Json<Value>, Status> {
    let auth_result = auth_headers.validate_with_body(&message, &slack_params.signing_secret);
    if auth_result.is_err() {
        info!("Failed to verify signature");
    }
    match serde_json::from_str(&message) {
        Ok(Value::Object(message_map)) => {
            check_token(&message_map, &slack_params).map_err(|e| {
                info!("{}", e);
                Status::BadRequest
            })?;
            match message_map.get("type").and_then(|type_val| type_val.as_str()) {
                Some("url_verification") => message_map.get("challenge")
                    .and_then(|challenge_val| challenge_val.as_str())
                    .and_then(|challenge_str| Some(Ok(Json(json!({"challenge": challenge_str})))))
                    .unwrap_or_else(|| Err(Status::BadRequest)),
                Some("event_callback") => {
                    match message_map.get("event") {
                        Some(Value::Object(event_obj)) =>
                            handle_event_object(event_obj, &slack_params, collector.inner()).map_err(|e| {
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
        _ => {
            info!("Got invalid request, full body '{}'", &message);
            Err(Status::BadRequest)
        }
    }
}


#[get("/app_status")]
fn app_status() -> Status {
    Status::Ok
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

impl SlackParams {
    fn from_env(is_prod: bool) -> SlackParams {
        fn get_env_var(name: &str) -> String {
            env::var(name).expect(&format!("Unable to access env var {}", name))
        }
        let regex = Regex::new(&get_regex_string()).unwrap();
        if is_prod {
            SlackParams {
                verification_token: get_env_var("SLACK_VERIFICATION_TOKEN"),
                app_id: get_env_var("SLACK_APP_ID"),
                client_id: get_env_var("SLACK_CLIENT_ID"),
                client_secret: get_env_var("SLACK_CLIENT_SECRET"),
                signing_secret: VerificationKey::new(&SHA256, get_env_var("SLACK_SIGNING_SECRET").as_bytes()),
                gocd_bod_id: get_env_var("GOCD_BOD_ID"),
                instance_token: get_env_var("SLACK_INSTANCE_TOKEN"),
                title_match_regex: regex,
            }
        }
        else {
            SlackParams {
                verification_token: "test".to_string(),
                app_id: "test".to_string(),
                client_id: "test".to_string(),
                client_secret: "test".to_string(),
                signing_secret: VerificationKey::new(&SHA256, "test".as_bytes()),
                gocd_bod_id: "test".to_string(),
                instance_token: "test".to_string(),
                title_match_regex: regex,
            }
        }
    }
}

fn main() {
    #[cfg(not(debug_assertions))]
    init_logging();
    let app = rocket::ignite();
    let is_prod = app.config().environment.is_prod();
    let slack_params = SlackParams::from_env(is_prod);
    app
        .mount("/", routes![message_receive, app_status])
        .manage(BuildInfoManager::new(&slack_params.instance_token))
        .manage(slack_params)
        .launch();
}
