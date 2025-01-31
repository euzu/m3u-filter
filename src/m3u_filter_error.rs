use std::error::Error;
use std::fmt::{Display, Formatter, Result};

#[macro_export]
macro_rules! get_errors_notify_message {
    ($errors:expr, $size:expr) => {
        if $errors.is_empty() {
            None
        } else {
            let text = $errors
                .iter()
                .filter(|&err| err.kind == M3uFilterErrorKind::Notify)
                .map(|err| err.message.as_str())
                .collect::<Vec<&str>>()
                .join("\r\n");
            if $size > 0 && text.len() > std::cmp::max($size - 3, 3) {
                Some(format!("{}...", text.get(0..$size).unwrap()))
            } else {
                Some(text)
            }
        }
    };
}

#[macro_export]
macro_rules! notify_err {
    ($text:expr) => {
        M3uFilterError::new(M3uFilterErrorKind::Notify, $text)
    };
}

#[macro_export]
macro_rules! info_err {
    ($text:expr) => {
        M3uFilterError::new(M3uFilterErrorKind::Info, $text)
    };
}

#[derive(Debug, PartialEq, Eq)]
pub enum M3uFilterErrorKind {
    // do not send with messaging
    Info,
    Notify, // send with messaging
}

#[derive(Debug)]
pub struct M3uFilterError {
    pub kind: M3uFilterErrorKind,
    pub message: String,
}

impl M3uFilterError {
    pub const fn new(kind: M3uFilterErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl Display for M3uFilterError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "M3uFilter error: {}", self.message)
    }
}

impl Error for M3uFilterError {}

pub fn to_io_error<E>(err: E) -> std::io::Error
where E: std::error::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}

pub fn str_to_io_error(err: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}
