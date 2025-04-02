use aws_sdk_ec2::error::SdkError;
use std::error::Error as StdError;
use std::fmt::{self};

// A simple error type that just contains a message
#[derive(Debug)]
pub struct AwsError {
    message: String,
}

impl AwsError {
    // Create a new error with a message
    pub fn new<S: Into<String>>(message: S) -> Self {
        AwsError {
            message: message.into(),
        }
    }

    // Create an error from another error
    pub fn from_err<E: fmt::Display>(message: &str, err: E) -> Self {
        AwsError {
            message: format!("{}: {}", message, err),
        }
    }
}

// Required for custom error types
impl fmt::Display for AwsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for AwsError {}

// Helper function for any SDK error
impl<E> From<SdkError<E>> for AwsError {
    fn from(err: SdkError<E>) -> Self {
        AwsError::new(format!("AWS SDK error: {}", err))
    }
}
