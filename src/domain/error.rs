use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    MissingBackend(String),
    InvalidDocument(String),
    Cancelled,
    Timeout,
    OutputLimit,
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingBackend(error) => write!(
                f,
                "Cannot start Poppler: {error}. Install pdfinfo and pdftoppm; see README."
            ),
            Self::InvalidDocument(error) => f.write_str(error),
            Self::Cancelled => f.write_str("Operation cancelled."),
            Self::Timeout => f.write_str("PDF processing exceeded 30 seconds."),
            Self::OutputLimit => f.write_str("PDF metadata exceeds the supported size."),
        }
    }
}
impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<String> for AppError {
    fn from(error: String) -> Self {
        Self::InvalidDocument(error)
    }
}
impl From<&str> for AppError {
    fn from(error: &str) -> Self {
        Self::InvalidDocument(error.into())
    }
}
