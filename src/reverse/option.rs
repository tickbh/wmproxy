use serde::{Serialize, Deserialize};
use wenmeng::FileServer;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseOption {
    pub file_server: FileServer,
}
