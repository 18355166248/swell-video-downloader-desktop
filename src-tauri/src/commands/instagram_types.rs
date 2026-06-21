use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstagramCollectMode {
    Single,
    DetailNext,
    ProfileRecent,
    StoryExperimental,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct CollectInstagramTargetsRequest {
    pub url: String,
    pub mode: InstagramCollectMode,
    pub count: u32,
    pub sessionid: Option<String>,
    pub cookie_file_path: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct InstagramCollectItem {
    pub url: String,
    pub kind: String,
    pub source_label: String,
    pub thumbnail_hint: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct CollectInstagramTargetsResponse {
    pub items: Vec<InstagramCollectItem>,
    pub resolved_count: usize,
    pub warnings: Vec<String>,
    pub cookie_bridge_file_path: Option<String>,
}
