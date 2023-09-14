use serde::{
    Deserialize,
    Serialize,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedObject {
    pub api_version: String,
    pub kind: String,
    pub pod_spec_path: String,
    pub watched_fields: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerConfig {
    pub tracked_objects: Vec<TrackedObject>,
}
