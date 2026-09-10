use crate::{cli::error::Result, cli_try};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
use ulid::Ulid;

pub fn temporary(output: &Path, extension: &OsStr) -> Result<(PathBuf, File)> {
    let parent = output.parent().unwrap_or(Path::new("."));
    let mut name = OsString::from(format!(".opencut-{}.", Ulid::generate()));
    // Keep the extension so media backends can select their output container.
    name.push(extension);
    let temp = parent.join(name);
    let file = cli_try!(
        OpenOptions::new().write(true).create_new(true).open(&temp),
        "io_error",
        "",
        6
    );
    Ok((temp, file))
}

pub fn commit(temp: &Path, output: &Path, overwrite: bool) -> Result<()> {
    let file = cli_try!(OpenOptions::new().write(true).open(temp), "io_error", "", 6);
    cli_try!(file.sync_all(), "io_error", "", 6);
    if overwrite {
        cli_try!(fs::rename(temp, output), "io_error", "", 6);
    } else {
        cli_try!(fs::hard_link(temp, output), "io_error", "", 6);
    }
    Ok(())
}

pub fn cleanup(temp: &Path, result: Result<()>) -> Result<()> {
    if temp.exists() {
        cli_try!(fs::remove_file(temp), "io_error", "", 6);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn publishes_new_output_and_cleans_staging_file() {
        let dir = Temp::new();
        let output = dir.0.join("video.mov");
        let (temp, mut file) = temporary(&output, output.extension().unwrap()).unwrap();
        assert_eq!(temp.extension(), output.extension());
        file.write_all(b"complete output").unwrap();
        drop(file);
        cleanup(&temp, commit(&temp, &output, false)).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"complete output");
        assert!(!temp.exists());
    }

    #[test]
    fn publication_does_not_replace_a_destination_created_after_staging() {
        let dir = Temp::new();
        let output = dir.0.join("timeline.json");
        let (temp, file) = temporary(&output, OsStr::new("tmp")).unwrap();
        drop(file);
        fs::write(&output, b"keep me").unwrap();
        let error = cleanup(&temp, commit(&temp, &output, false)).unwrap_err();
        assert_eq!(error.code, "io_error");
        assert_eq!(error.exit, 6);
        assert!(!error.file.is_empty());
        assert!(error.line > 0);
        assert_eq!(fs::read(&output).unwrap(), b"keep me");
        assert!(!temp.exists());
    }

    #[test]
    fn document_overwrite_and_no_overwrite_keep_existing_behavior() {
        let dir = Temp::new();
        let output = dir.0.join("timeline.json");
        let original = serde_json::json!({"value": 1});
        let replacement = serde_json::json!({"value": 2});
        crate::cli::document::write_atomic(&output, &original, false).unwrap();
        let error = crate::cli::document::write_atomic(&output, &replacement, false).unwrap_err();
        assert_eq!(error.code, "io_error");
        assert_eq!(
            fs::read(&output).unwrap(),
            serde_json::to_vec_pretty(&original).unwrap()
        );
        crate::cli::document::write_atomic(&output, &replacement, true).unwrap();
        assert_eq!(
            fs::read(&output).unwrap(),
            serde_json::to_vec_pretty(&replacement).unwrap()
        );
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_write_cleans_staging_without_publishing() {
        let dir = Temp::new();
        let output = dir.0.join("video.mov");
        let (temp, file) = temporary(&output, OsStr::new("mov")).unwrap();
        drop(file);
        let error = crate::cli_error!("encode_failure", "", 5, "failed encoding");
        let error = cleanup(&temp, Err(error)).unwrap_err();
        assert_eq!(error.code, "encode_failure");
        assert!(!temp.exists());
        assert!(!output.exists());
    }

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("opencut-output-test-{}", Ulid::generate()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
