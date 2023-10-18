use serde::{Serialize, Deserialize};
use wenmeng::FileServer;

fn default_headers() -> Vec<Vec<String>> {
    vec![]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationConfig {
    pub rule: String,
    pub file_server: Option<FileServer>,
    #[serde(default = "default_headers")]
    pub headers: Vec<Vec<String>>,
    pub reverse_proxy: Option<String>,
}