//! CLI-owned file I/O for the shared timeline format.
pub use crate::timeline::TimelineSerialization as Document;
pub use crate::timeline::parse;
use crate::{cli::error::Result, cli_try};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use ulid::Ulid;

pub fn load(path: &Path) -> Result<(Value, Document)> {
    let contents = cli_try!(fs::read(path), "io_error", "", 6);
    let value: Value = cli_try!(serde_json::from_slice(&contents), "invalid_json", "", 3);
    let document = crate::timeline::parse(&value)?;
    Ok((value, document))
}

pub fn write_atomic(path: &Path, value: &Value, overwrite: bool) -> Result<()> {
    let bytes = cli_try!(serde_json::to_vec_pretty(value), "invalid_json", "", 3);
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".opencut-{}.tmp", Ulid::generate()));
    let result = (|| {
        let mut file = cli_try!(
            OpenOptions::new().write(true).create_new(true).open(&temp),
            "io_error",
            "",
            6
        );
        cli_try!(file.write_all(&bytes), "io_error", "", 6);
        cli_try!(file.sync_all(), "io_error", "", 6);
        if overwrite {
            cli_try!(fs::rename(&temp, path), "io_error", "", 6);
        } else {
            cli_try!(fs::hard_link(&temp, path), "io_error", "", 6);
        }
        Ok(())
    })();
    if temp.exists() {
        cli_try!(fs::remove_file(&temp), "io_error", "", 6);
    }
    result
}
