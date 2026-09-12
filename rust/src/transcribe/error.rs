use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    pub file: &'static str,
    pub line: u32,
}

impl Error {
    pub fn new(
        code: &'static str,
        message: impl fmt::Display,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            code,
            message: message.to_string(),
            file,
            line,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}:{})",
            self.code, self.message, self.file, self.line
        )
    }
}

impl std::error::Error for Error {}
