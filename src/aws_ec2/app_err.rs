use aws_sdk_ec2::error::SdkError;
use std::error::Error as StdError;
use std::fmt::{self};

#[derive(Debug)]
pub struct ApplicationError {
    message: String,
}

impl ApplicationError {
    pub fn new<S: Into<String>>(message: S) -> Self {
        ApplicationError {
            message: message.into(),
        }
    }

    pub fn from_err<E: std::fmt::Debug>(message: &str, err: E) -> Self {
        ApplicationError {
            message: format!("{}: {:?}", message, err),
        }
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for ApplicationError {}

impl<E: std::fmt::Debug> From<SdkError<E>> for ApplicationError {
    fn from(err: SdkError<E>) -> Self {
        ApplicationError::new(format!("AWS SDK error: {:?}", err))
    }
}
