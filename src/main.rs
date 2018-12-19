#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate log;

use std::env;
use rocket::*;
use rocket::http::*;
use serde_json::{Value, json};
use rocket_contrib::json::Json;
use regex::Regex;

mod slack;
use crate::slack::{SlackParams, handle_event_object, check_token};

mod build_info_manager;
use crate::build_info_manager::{BuildInfoManager};

#[cfg(test)]
mod test;

#[post("/event", data = "<message>")]
fn message_receive(message: Json<Value>, slack_params: State<SlackParams>, collector: State<BuildInfoManager>) -> Result<Json<Value>, Status> {
    match message.into_inner() {
        Value::Object(message_map) => {
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
                        Some(Value::Object(event_obj)) => handle_event_object(event_obj, &slack_params, collector.inner()).map_err(|e| {
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
        let regex = Regex::new(
            r"^Go pipeline stage \[(?P<stage_name>[\w_]+)/(?P<build_num>\d+)/(?P<step_name>\w+)/(?P<number>\d+)\] passed"
        ).unwrap();
        if is_prod {
            SlackParams {
                verification_token: get_env_var("SLACK_VERIFICATION_TOKEN"),
                app_id: get_env_var("SLACK_APP_ID"),
                client_id: get_env_var("SLACK_CLIENT_ID"),
                client_secret: get_env_var("SLACK_CLIENT_SECRET"),
                signing_secret: get_env_var("SLACK_SIGNING_SECRET"),
                gocd_bod_id: get_env_var("GOCD_BOD_ID"),
                title_match_regex: regex,
            }
        }
        else {
            SlackParams {
                verification_token: "test".to_string(),
                app_id: "test".to_string(),
                client_id: "test".to_string(),
                client_secret: "test".to_string(),
                signing_secret: "test".to_string(),
                gocd_bod_id: "test".to_string(),
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
    app
        .mount("/", routes![message_receive, app_status])
        .manage(SlackParams::from_env(is_prod))
        .manage(BuildInfoManager::new())
        .launch();
}
