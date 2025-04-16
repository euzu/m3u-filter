pub mod string_utils;
pub mod json_utils;
pub mod default_utils;
pub mod size_utils;
pub mod sys_utils;
pub mod hash_utils;
pub mod compression;
pub(crate) mod file;
pub(crate) mod network;
pub mod bincode_utils;
pub mod time_utils;
pub mod crypto_utils;

#[macro_export]
macro_rules! debug_if_enabled {
    ($fmt:expr, $( $args:expr ),*) => {
        if log::log_enabled!(log::Level::Debug) {
            log::log!(log::Level::Debug, $fmt, $($args),*);
        }
    };

    ($txt:expr) => {
        if log::log_enabled!(log::Level::Debug) {
            log::log!(Level::Debug, $txt);
        }
    };
}

#[macro_export]
macro_rules! trace_if_enabled {
    ($fmt:expr, $( $args:expr ),*) => {
        if log::log_enabled!(log::Level::Trace) {
            log::log!(log::Level::Trace, $fmt, $($args),*);
        }
    };

    ($txt:expr) => {
        if log::log_enabled!(log::Level::Trace) {
            log::log!(Level::Trace, $txt);
        }
    };
}

pub use debug_if_enabled;
pub use trace_if_enabled;