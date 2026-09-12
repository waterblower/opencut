pub use anyhow::{Error, Result, anyhow};

#[macro_export]
macro_rules! cli_error {
    ($code:expr, $pointer:expr, $exit:expr, $($arg:tt)*) => {
        $crate::cli::error::anyhow!(
            "{}: {} ({}) at {}:{}",
            $code, format!($($arg)*), $pointer, file!(), line!()
        )
    };
}

#[macro_export]
macro_rules! cli_try {
    ($value:expr, $code:expr, $pointer:expr, $exit:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return Err($crate::cli_error!($code, $pointer, $exit, "{error:#}")),
        }
    };
}
