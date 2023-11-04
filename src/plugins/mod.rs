mod file_server;
mod plugin_trait;

pub use file_server::FileServer;

fn calc_file_size(len: u64) -> String {
    if len < 1024 {
        return format!("{}B", len);
    } else if len < 1024 * 1024 {
        return format!("{}K", len / 1024);
    } else if len < 1024 * 1024 * 1024 {
        return format!("{}M", len / (1024 * 1024));
    } else {
        return format!("{}G", len / (1024 * 1024 * 1024));
    }
}