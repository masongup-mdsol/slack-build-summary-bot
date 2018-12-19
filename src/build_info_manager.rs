
pub trait AcceptBuildInfo {
    fn new_build_message(&self, stage_name: &str, build_num: u32);
}

pub struct BuildInfoManager {
}

impl BuildInfoManager {
    pub fn new() -> BuildInfoManager {
        BuildInfoManager {}
    }
}

impl AcceptBuildInfo for BuildInfoManager {
    fn new_build_message(&self, _stage_name: &str, _build_num: u32) {
    }
}
