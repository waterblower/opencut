pub use crate::transcribe::{Format, Options, transcribe};
use crate::{cli::error::Result, cli_error, cli_try};
use std::path::Path;
use tokio::fs;

/// Check before the request and again before writing so output cannot replace input.
pub async fn check_output(input: &Path, output: &Path, overwrite: bool) -> Result<()> {
    if !cli_try!(fs::try_exists(output).await, "io_error", "", 6) {
        return Ok(());
    }
    let source = cli_try!(fs::canonicalize(input).await, "io_error", "", 6);
    let target = cli_try!(fs::canonicalize(output).await, "io_error", "", 6);
    if source == target {
        return Err(cli_error!(
            "output_is_source",
            "",
            6,
            "output would overwrite input media"
        ));
    }
    if !overwrite {
        return Err(cli_error!(
            "output_exists",
            "",
            6,
            "use --overwrite to replace {}",
            output.display()
        ));
    }
    Ok(())
}
