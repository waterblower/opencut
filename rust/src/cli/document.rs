//! CLI-owned file I/O for the shared timeline format.
pub use crate::timeline::TimelineSerialization as Document;
pub use crate::timeline::parse;
use anyhow::{Context as _, Result};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use ulid::Ulid;

pub fn load(path: &Path) -> Result<(Value, Document)> {
    let contents = fs::read(path).context(format!("io_error at {}:{}", file!(), line!()))?;
    let value: Value = serde_json::from_slice(&contents).context(format!(
        "invalid_json at {}:{}",
        file!(),
        line!()
    ))?;
    let document = crate::timeline::parse(&value)?;
    Ok((value, document))
}

pub fn write_atomic(path: &Path, value: &Value, overwrite: bool) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context(format!(
        "invalid_json at {}:{}",
        file!(),
        line!()
    ))?;
    write_bytes(path, &bytes, overwrite)
}

/// Publish transcript bytes without blocking the async runtime. Reuses the same
/// atomic publication and cleanup as synchronous document writes.
pub async fn write_atomic_bytes(path: &Path, bytes: Vec<u8>, overwrite: bool) -> Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || write_bytes(&path, &bytes, overwrite))
        .await
        .context(format!("io_error at {}:{}", file!(), line!()))?
}

fn write_bytes(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".opencut-{}.tmp", Ulid::generate()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        file.write_all(bytes)
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        file.sync_all()
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        if overwrite {
            fs::rename(&temp, path).context(format!("io_error at {}:{}", file!(), line!()))?;
        } else {
            fs::hard_link(&temp, path).context(format!("io_error at {}:{}", file!(), line!()))?;
        }
        Ok(())
    })();
    if temp.exists() {
        fs::remove_file(&temp).context(format!("io_error at {}:{}", file!(), line!()))?;
    }
    result
}
