use std::sync::{Mutex, RwLock};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use slack_api::chat::{post_message, PostMessageRequest, update, UpdateRequest};
use slack_api::requests::{default_client, Client};
use chrono::prelude::*;
use time::Duration;

pub trait AcceptBuildInfo {
    fn new_build_message(&self, stage_name: &str, build_num: u32, build_step: &str, pass_fail: &str);
}

pub struct BuildInfoManager {
    message_index: Mutex<HashMap<BuildInfoIndex, BuildInfoEntry>>,
    slack_instance_token: String,
    slack_client: Client,
    last_cleanout_time: RwLock<DateTime<Utc>>,
}

struct BuildInfoEntry {
    failed: bool,
    slack_timestamp: String,
    last_update_time: DateTime<Utc>,
}

#[derive(Hash, PartialEq, Eq)]
struct BuildInfoIndex {
    stage_name: String,
    build_num: u32,
}

impl BuildInfoManager {
    pub fn new(slack_token: &str) -> BuildInfoManager {
        BuildInfoManager {
            message_index: Mutex::new(HashMap::new()),
            slack_instance_token: slack_token.to_string(),
            slack_client: default_client().unwrap(),
            last_cleanout_time: RwLock::new(Utc::now())
        }
    }

    fn clear_old_message_entries(&self) {
        match self.last_cleanout_time.try_read() {
            Err(_) => return,
            Ok(time) => {
                if Utc::now().signed_duration_since(*time) < Duration::days(1) {
                    return;
                }
            }
        }

        let mut message_index = self.message_index.lock().unwrap();
        message_index.retain(|_, entry| Utc::now().signed_duration_since(entry.last_update_time) < Duration::hours(4));
        let mut mutable_cleanout_time = self.last_cleanout_time.write().unwrap();
        *mutable_cleanout_time = Utc::now();
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;

    #[test]
    fn test_clear_old_message_entries() {
        let manager = BuildInfoManager::new("test_token");
        {
            let mut cleanout_time = manager.last_cleanout_time.write().unwrap();
            *cleanout_time = Utc::now() - Duration::days(2);
            let mut index_map = manager.message_index.lock().unwrap();
            index_map.insert(
                BuildInfoIndex { stage_name: "test".to_string(), build_num: 1 },
                BuildInfoEntry {
                    failed: false, slack_timestamp: "test".to_string(), last_update_time: Utc::now() - Duration::hours(1)
                }
            );
            index_map.insert(
                BuildInfoIndex { stage_name: "test".to_string(), build_num: 2 },
                BuildInfoEntry {
                    failed: false, slack_timestamp: "test".to_string(), last_update_time: Utc::now() - Duration::days(1)
                }
            );
            assert_eq!(index_map.len(), 2);
        }
        manager.clear_old_message_entries();
        let index_map = manager.message_index.lock().unwrap();
        assert_eq!(index_map.len(), 1);
    }
}

impl AcceptBuildInfo for BuildInfoManager {
    fn new_build_message(&self, stage_name: &str, build_num: u32, build_step: &str, pass_fail: &str) {
        if !stage_name.starts_with("Delorean") {
            return;
        }
        let index = BuildInfoIndex { stage_name: stage_name.to_string(), build_num: build_num };
        let failed = pass_fail == "failed";
        info!("Handling build message for {}", &stage_name);
        match self.message_index.lock().unwrap().entry(index) {
            Entry::Vacant(entry) => {
                let request = PostMessageRequest {
                    channel: "#gocd-notifications",
                    text: &format!("GoCD Build for stage {} has reached step {} and {}", &stage_name, &build_step, &pass_fail),
                    ..Default::default()
                };
                info!("About to try to create new moessage with text: '{}'", &request.text);
                match post_message(&self.slack_client, &self.slack_instance_token, &request) {
                    Ok(response) => {
                        if let Some(timestamp) = response.ts {
                            entry.insert(BuildInfoEntry {
                                failed,
                                slack_timestamp: timestamp,
                                last_update_time: Utc::now()
                            });
                        }
                    },
                    Err(error) => error!("Got Slack Post error: {:?}", error),
                }
            },
            Entry::Occupied(mut entry) => {
                let mut info_entry = entry.get_mut();
                let request = UpdateRequest {
                    ts: &info_entry.slack_timestamp,
                    channel: "#gocd-notifications",
                    text: &format!("GoCD Build for stage {} has reached step {} and {}", &stage_name, &build_step, &pass_fail),
                    as_user: Some(true),
                    ..Default::default()
                };
                info!("About to try to update moessage with text: '{}'", &request.text);
                match update(&self.slack_client, &self.slack_instance_token, &request) {
                    Err(error) => error!("Got Slack Update error: {:?}", error),
                    Ok(_) => {
                        info_entry.last_update_time = Utc::now();
                        info_entry.failed = failed;
                    }
                }
            }
        }
        self.clear_old_message_entries();
    }
}
