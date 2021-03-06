use std::sync::{Mutex, RwLock};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use slack_api::chat::{post_message, PostMessageRequest, update, UpdateRequest};
use slack_api::requests::{default_client, Client};
use chrono::prelude::*;
use time::Duration;

use crate::gocd::GoCDInfo;

pub trait AcceptBuildInfo {
    fn new_build_message(&self, stage_name: &str, build_num: u64, build_step: &str, pass_fail: &str);
}

pub struct BuildInfoManager {
    message_index: Mutex<HashMap<BuildInfoIndex, BuildInfoEntry>>,
    slack_instance_token: String,
    slack_client: Client,
    last_cleanout_time: RwLock<DateTime<Utc>>,
    info_monitors: Vec<BuildInfoMonitor>,
    gocd_talker: GoCDInfo
}

struct BuildInfoEntry {
    failed: bool,
    slack_timestamp: String,
    last_update_time: DateTime<Utc>,
}

struct BuildInfoMonitor {
    name: String,
    filter_prefix: String,
    post_channel: String,
}

#[derive(Hash, PartialEq, Eq)]
struct BuildInfoIndex {
    monitor_name: String,
    git_index: u64,
}

impl BuildInfoManager {
    pub fn new(slack_token: &str, gocd_token: &str) -> BuildInfoManager {
        BuildInfoManager {
            message_index: Mutex::new(HashMap::new()),
            slack_instance_token: slack_token.to_string(),
            slack_client: default_client().unwrap(),
            last_cleanout_time: RwLock::new(Utc::now()),
            info_monitors: vec![
                BuildInfoMonitor {
                    name: "Delorean".to_string(),
                    filter_prefix: "Delorean_".to_string(),
                    post_channel: "CCDJ9UWAZ".to_string(),
                }
            ],
            gocd_talker: GoCDInfo::create(&gocd_token),
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

    fn process_build_message(&self, index: BuildInfoIndex, message_text: &str, post_channel: &str, failed: bool) {
        match self.message_index.lock().unwrap().entry(index) {
            Entry::Vacant(entry) => {
                let request = PostMessageRequest {
                    channel: &post_channel,
                    text: message_text,
                    ..Default::default()
                };
                info!("About to try to create new message with text: '{}'", &request.text);
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
                    channel: &post_channel,
                    text: message_text,
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
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;

    #[test]
    fn test_clear_old_message_entries() {
        let manager = BuildInfoManager::new("test_token", "test_gocd");
        {
            let mut cleanout_time = manager.last_cleanout_time.write().unwrap();
            *cleanout_time = Utc::now() - Duration::days(2);
            let mut index_map = manager.message_index.lock().unwrap();
            index_map.insert(
                BuildInfoIndex { monitor_name: "test".to_string(), git_index: 1 },
                BuildInfoEntry {
                    failed: false, slack_timestamp: "test".to_string(), last_update_time: Utc::now() - Duration::hours(1)
                }
            );
            index_map.insert(
                BuildInfoIndex { monitor_name: "test".to_string(), git_index: 2 },
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
    fn new_build_message(&self, stage_name: &str, build_num: u64, build_step: &str, pass_fail: &str) {
        if let Some(monitor) = self.info_monitors.iter().find(|im| stage_name.starts_with(&im.filter_prefix)) {
            match self.gocd_talker.get_history(stage_name) {
                Err(err_str) => error!("Error getting GoCD Info: {}", err_str),
                Ok(history_vec) => {
                    match history_vec.iter().find(|hv| hv.counter == build_num) {
                        None => error!("Could not find build info in GoCD!"),
                        Some(history_item) => {
                            let index = BuildInfoIndex {
                                monitor_name: monitor.name.clone(),
                                git_index: history_item.id,
                            };
                            let failed = pass_fail == "failed";
                            info!("Handling build message for {}", &stage_name);
                            let message_text = &format!("GoCD Build for {} has reached step {} on {} and {}",
                                   &monitor.name, &build_step, &stage_name, &pass_fail);
                            self.process_build_message(index, &message_text, &monitor.post_channel, failed);
                        }
                    }
                }
            }
        }
        self.clear_old_message_entries();
    }

}

