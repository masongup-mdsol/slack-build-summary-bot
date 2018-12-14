#![feature(proc_macro_hygiene, decl_macro)]

use rocket::*;
use rocket::http::*;
use serde_derive::Deserialize;
use rocket_contrib::json::Json;
use log::{info};

#[derive(Deserialize)]
struct UrlVerify {
    token: String,
    challenge: String,
    #[serde(rename = "type")]
    request_type: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Message {
    #[serde(rename = "type")]
    request_type: String,
    channel: String,
    user: String,
    text: String,
    ts: String,
    edited: Option<MessageEdit>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MessageEdit {
    user: String,
    ts: String,
}

#[post("/event", data = "<challenge>", rank = 2)]
fn challenge_receive(challenge: Json<UrlVerify>) -> String {
    info!("Received url verify request with token {} and type {}", &challenge.token, &challenge.request_type);
    challenge.challenge.clone()
}

#[post("/event", data = "<message>")]
fn message_receive(message: Json<Message>) {
    info!("Received message with text '{}' by user '{}'", &message.text, &message.user);
}

#[get("/app_status")]
fn app_status() -> Status {
    Status::Ok
}

fn main() {
    rocket::ignite()
        .mount("/", routes![challenge_receive, message_receive, app_status])
        .launch();
}
