use std::{
    path::Path,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let Some(scripts) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() else {
        eprintln!("Missing scripts directory at {}:{}", file!(), line!());
        return ExitCode::FAILURE;
    };
    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
        command.arg(scripts.join("cargo-windows.ps1"));
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("bash");
        command.arg(scripts.join("cargo-macos.sh"));
        command
    } else {
        eprintln!(
            "Supported platforms are Windows and macOS at {}:{}",
            file!(),
            line!()
        );
        return ExitCode::FAILURE;
    };
    command.args(std::env::args_os().skip(1));
    match command.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!(
                "Could not run platform script at {}:{}: {error}",
                file!(),
                line!()
            );
            ExitCode::FAILURE
        }
    }
}
