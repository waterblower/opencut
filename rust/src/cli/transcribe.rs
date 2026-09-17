pub use crate::transcribe::{Format, Options, transcribe_response};
use anyhow::{Context as _, Result, anyhow};
use std::path::Path;
use tokio::fs;

/// Check before the request and again before writing so output cannot replace input.
pub async fn check_output(input: &Path, output: &Path, overwrite: bool) -> Result<()> {
    if !fs::try_exists(output)
        .await
        .context(format!("io_error at {}:{}", file!(), line!()))?
    {
        return Ok(());
    }
    let source =
        fs::canonicalize(input)
            .await
            .context(format!("io_error at {}:{}", file!(), line!()))?;
    let target =
        fs::canonicalize(output)
            .await
            .context(format!("io_error at {}:{}", file!(), line!()))?;
    if source == target {
        return Err(anyhow!(
            "output_is_source: output would overwrite input media at {}:{}",
            file!(),
            line!()
        ));
    }
    if !overwrite {
        return Err(anyhow!(
            "output_exists: use --overwrite to replace {} at {}:{}",
            output.display(),
            file!(),
            line!()
        ));
    }
    Ok(())
}
