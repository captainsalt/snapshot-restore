use aws_sdk_ec2::error::SdkError;
use std::error::Error as StdError;
use std::fmt::{self};

// A simple error type that just contains a message
#[derive(Debug)]
pub struct ApplicationError {
    message: String,
}

impl ApplicationError {
    // Create a new error with a message
    pub fn new<S: Into<String>>(message: S) -> Self {
        ApplicationError {
            message: message.into(),
        }
    }

    // Create an error from another error
    pub fn from_err<E: std::fmt::Debug>(message: &str, err: E) -> Self {
        ApplicationError {
            message: format!("{}: {:?}", message, err),
        }
    }
}

// Required for custom error types
impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for ApplicationError {}

// Helper function for any SDK error
impl<E> From<SdkError<E>> for ApplicationError {
    fn from(err: SdkError<E>) -> Self {
        ApplicationError::new(format!("AWS SDK error: {}", err))
    }
}
