use serde::{Serialize, Deserialize};
use wenmeng::FileServer;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseOption {
    pub file_server: Option<FileServer>,
}

impl ReverseOption {
    
    pub fn fix_default(&mut self) {
        if let Some(f) = &mut self.file_server {
            f.pre_deal_request();
        }
    }
}