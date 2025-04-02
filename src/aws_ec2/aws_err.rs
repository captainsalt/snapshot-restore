use aws_sdk_ec2::error::SdkError;
use std::error::Error as StdError;
use std::fmt::{self};

// A simple error type that just contains a message
#[derive(Debug)]
pub struct AppError {
    message: String,
}

impl AppError {
    // Create a new error with a message
    pub fn new<S: Into<String>>(message: S) -> Self {
        AppError {
            message: message.into(),
        }
    }

    // Create an error from another error
    pub fn from_err<E: fmt::Display + std::fmt::Debug>(message: &str, err: E) -> Self {
        AppError {
            message: format!("{}: {:?}", message, err),
        }
    }
}

// Required for custom error types
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for AppError {}

// Helper function for any SDK error
impl<E> From<SdkError<E>> for AppError {
    fn from(err: SdkError<E>) -> Self {
        AppError::new(format!("AWS SDK error: {}", err))
    }
}
