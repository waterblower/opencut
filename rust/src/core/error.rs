use serde::Serialize;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: String,
    pub pointer: String,
    pub message: String,
    pub file: &'static str,
    pub line: u32,
    #[serde(skip)]
    pub exit: u8,
}

impl Error {
    pub fn new(
        code: &str,
        pointer: &str,
        message: impl fmt::Display,
        exit: u8,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            code: code.into(),
            pointer: pointer.into(),
            message: message.to_string(),
            file,
            line,
            exit,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}, {}:{})",
            self.code, self.message, self.pointer, self.file, self.line
        )
    }
}

impl std::error::Error for Error {}

#[macro_export]
macro_rules! cli_error {
    ($code:expr, $pointer:expr, $exit:expr, $($arg:tt)*) => {
        $crate::core::error::Error::new($code, $pointer, format!($($arg)*), $exit, file!(), line!())
    };
}

#[macro_export]
macro_rules! cli_try {
    ($value:expr, $code:expr, $pointer:expr, $exit:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return Err($crate::cli_error!($code, $pointer, $exit, "{error}")),
        }
    };
}
