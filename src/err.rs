//! All blight library related errors in one place. See [`Error`] and [`ErrorKind`]

use std::fmt::Display;

/// Result type alias with blight [`Error`] as the default error type
///
/// This type alias is the same as [`Result`] provided in this module, which is now the preferred alias.
pub type BlResult<T> = Result<T>;

/// Result type alias with blight [`Error`] as the default error type
pub type Result<T> = std::result::Result<T, Error>;

/// Error type containing possible error source and the [`ErrorKind`]
///
/// Use [`Error::kind`] to distinguish between different types of errors.
/// Use [`std::error::Error::source`] to get the error source (if it is present).
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<std::io::Error>,
}

impl Error {
    pub(crate) fn with_source(mut self, source: std::io::Error) -> Self {
        self.source.replace(source);
        self
    }

    /// Get the [`ErrorKind`] to distinguish between different error types
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl From<ErrorKind> for Error {
    fn from(value: ErrorKind) -> Self {
        Self {
            kind: value,
            source: None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)?;
        if let Some(s) = &self.source {
            write!(f, " (source: {s})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e as _)
    }
}

/// Different kinds of possible errors
///
/// The `Display` trait impl provides human-friendly, descriptive messages for each variant.
#[derive(Debug, Clone)]
pub enum ErrorKind {
    ReadDir { dir: &'static str },
    ReadMax,
    ReadCurrent,
    WriteValue { device: String },
    ValueTooLarge { given: u32, supported: u32 },
    SweepError,
    NotFound,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::ReadDir { dir } => write!(f, "failed to read {dir} directory",),
            ErrorKind::NotFound => write!(f, "no known backlight or LED device detected"),
            ErrorKind::WriteValue { device } => {
                write!(
                    f,
                    "failed to write to the brightness file of device '{device}'",
                )
            }
            ErrorKind::ReadCurrent => write!(f, "failed to read current brightness value"),
            ErrorKind::ReadMax => write!(f, "failed to read max brightness value"),
            ErrorKind::SweepError => {
                write!(f, "failed to perform a sweep-write on the brightness file")
            }
            ErrorKind::ValueTooLarge { given, supported } => write!(
                f,
                "provided value '{given}' is larger than the max supported value of '{supported}'"
            ),
        }
    }
}
