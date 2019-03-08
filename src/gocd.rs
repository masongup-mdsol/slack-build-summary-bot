use reqwest::header;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use std::io::Read;
use std::time::Duration;

pub struct GoCDInfo {
    client: reqwest::Client
}

impl GoCDInfo {
    pub fn create(auth_str: &str) -> GoCDInfo {
        let cert = read_cert();
        let mut headers = header::HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            http::header::HeaderValue::from_str(&format!("Basic {}", auth_str)).unwrap()
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .add_root_certificate(cert)
            .danger_accept_invalid_hostnames(true)
            .timeout(Duration::from_secs(1))
            .build().unwrap();
        GoCDInfo { client }
    }

    pub fn get_history(&self, pipeline_name: &str) -> Result<Vec<HistoryItem>, String> {
        let url = format!("https://gocd.imedidata.com:8154/go/api/pipelines/{}/history", pipeline_name);
        let response: serde_json::Value = self.client.get(&url)
            .header(ACCEPT, "application/vnd.go.cd.v6+json")
            .send().map_err(|e| format!("Request Error: {}", e))?
            .json().map_err(|e| format!("JSON parse error: {}", e))?;
        let pipelines_json_array = response.get("pipelines")
            .and_then(|p| p.as_array())
            .ok_or("Invalid Json")?;
        Ok(pipelines_json_array.iter().filter_map(|p| HistoryItem::from_json(&p)).collect())
    }
}

#[derive(Debug)]
pub struct HistoryItem {
    pub counter: u64,
    pub id: u64,
}

impl HistoryItem {
    fn from_json(json_obj: &serde_json::Value) -> Option<HistoryItem> {
        let maybe_id = json_obj.pointer("/build_cause/material_revisions/0/modifications/0/id")
            .and_then(|id| id.as_u64());
        let maybe_counter = json_obj.get("counter").and_then(|c| c.as_u64());
        if maybe_id.is_some() && maybe_counter.is_some() {
            return Some(HistoryItem {
                counter: maybe_counter.unwrap(),
                id: maybe_id.unwrap()
            })
        }
        else {
            return None
        }
    }
}

fn read_cert() -> reqwest::Certificate {
    let mut cert_buff = vec![];
    std::fs::File::open("gocd_cert.pem").unwrap().read_to_end(&mut cert_buff).unwrap();
    reqwest::Certificate::from_pem(&cert_buff).unwrap()
}
