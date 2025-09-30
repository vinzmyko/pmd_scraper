use std::{fs, path::Path};

use serde_json::json;

pub fn write_progress(path: &Path, current: usize, total: usize, phase: &str, status: &str) {
    let json = json!({
        "current": current,
        "total": total,
        "phase": phase,
        "status": status,
    });
    let _ = fs::write(path, json.to_string());
}
