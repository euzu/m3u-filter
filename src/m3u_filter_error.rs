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
                .filter(|&err| err.kind == TuliProxErrorKind::Notify)
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

pub use get_errors_notify_message;

#[macro_export]
macro_rules! notify_err {
    ($text:expr) => {
        TuliProxError::new(TuliProxErrorKind::Notify, $text)
    };
}

pub use notify_err;

#[macro_export]
macro_rules! info_err {
    ($text:expr) => {
        TuliProxError::new(TuliProxErrorKind::Info, $text)
    };
}
pub use info_err;


#[macro_export]
macro_rules! create_tuliprox_error {
     ($kind: expr, $($arg:tt)*) => {
        TuliProxError::new($kind, format!($($arg)*))
    }
}
pub use create_tuliprox_error;

#[macro_export]
macro_rules! create_tuliprox_error_result {
     ($kind: expr, $($arg:tt)*) => {
        Err(TuliProxError::new($kind, format!($($arg)*)))
    }
}
pub use create_tuliprox_error_result;

#[macro_export]
macro_rules! handle_tuliprox_error_result_list {
    ($kind:expr, $result: expr) => {
        let errors = $result
            .filter_map(|result| {
                if let Err(err) = result {
                    Some(err.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        if !&errors.is_empty() {
            return Err(TuliProxError::new($kind, errors.join("\n")));
        }
    }
}

pub use handle_tuliprox_error_result_list;

#[macro_export]
macro_rules! handle_tuliprox_error_result {
    ($kind:expr, $result: expr) => {
        if let Err(err) = $result {
            return Err(TuliProxError::new($kind, err.to_string()));
        }
    }
}

pub use handle_tuliprox_error_result;


#[derive(Debug, PartialEq, Eq)]
pub enum TuliProxErrorKind {
    // do not send with messaging
    Info,
    Notify, // send with messaging
}

#[derive(Debug)]
pub struct TuliProxError {
    pub kind: TuliProxErrorKind,
    pub message: String,
}

impl TuliProxError {
    pub const fn new(kind: TuliProxErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl Display for TuliProxError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "TuliProx error: {}", self.message)
    }
}

impl Error for TuliProxError {}

pub fn to_io_error<E>(err: E) -> std::io::Error
where
    E: std::error::Error,
{ std::io::Error::new(std::io::ErrorKind::Other, err.to_string()) }

pub fn str_to_io_error(err: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}
