//! CLI-owned file I/O for the shared timeline format.
pub use crate::timeline::TimelineSerialization as Document;
pub use crate::timeline::parse;
use crate::{
    cli::{error::Result, output},
    cli_try,
};
use serde_json::Value;
use std::{fs, io::Write, path::Path};

pub fn load(path: &Path) -> Result<(Value, Document)> {
    let contents = cli_try!(fs::read(path), "io_error", "", 6);
    let value: Value = cli_try!(serde_json::from_slice(&contents), "invalid_json", "", 3);
    let document = crate::timeline::parse(&value)?;
    Ok((value, document))
}

pub fn write_atomic(path: &Path, value: &Value, overwrite: bool) -> Result<()> {
    let bytes = cli_try!(serde_json::to_vec_pretty(value), "invalid_json", "", 3);
    let (temp, mut file) = output::temporary(path, std::ffi::OsStr::new("tmp"))?;
    let result = (|| {
        cli_try!(file.write_all(&bytes), "io_error", "", 6);
        output::commit(&temp, path, overwrite)
    })();
    drop(file);
    output::cleanup(&temp, result)
}
