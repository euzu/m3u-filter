use std::error::Error;
use std::fmt::{Display, Formatter, Result};

#[derive(Debug)]
pub(crate) struct M3uFilterError {
    message: String,
}

impl M3uFilterError {
    pub fn new(message: String) -> M3uFilterError {
        M3uFilterError {
            message,
        }
    }
}

impl Display for M3uFilterError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "M3uFilter error: {}", self.message)
    }
}

impl Error for M3uFilterError {}

