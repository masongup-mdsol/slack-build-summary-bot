#![feature(proc_macro_hygiene, decl_macro)]

use rocket::*;
use rocket::http::*;
use serde_json::{Value, json};
use rocket_contrib::json::Json;
use log::{info};

#[post("/event", data = "<message>")]
fn message_receive(message: Json<Value>) -> Result<Json<Value>, Status> {
    if let Value::Object(message_map) = message.into_inner() {
        //message_map.get("token").and_then(|token_val| token_val.as_str())
        match message_map.get("type").and_then(|type_val| type_val.as_str()) {
            Some("url_verification") => message_map.get("challenge")
                .and_then(|challenge_val| challenge_val.as_str())
                .and_then(|challenge_str| Some(Ok(Json(json!({"challenge": challenge_str})))))
                .unwrap_or_else(|| Err(Status::BadRequest)),
            _ => Err(Status::BadRequest),
        }
    }
    else {
        Err(Status::BadRequest)
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
