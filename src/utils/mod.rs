pub mod file_utils;
pub mod request_utils;
pub mod download;
pub mod string_utils;
pub mod json_utils;
pub mod config_reader;
pub mod default_utils;
pub mod multi_file_reader;
pub mod file_lock_manager;
pub mod compressed_file_reader;
mod compression_utils;
pub mod directed_graph;

#[macro_export]
macro_rules! debug_if_enabled {
    ($fmt:expr, $( $args:expr ),*) => {
        if log::log_enabled!(log::Level::Debug) {
            log::log!(Level::Debug, $fmt, $($args),*);
        }
    };

    ($txt:expr) => {
        if log::log_enabled!(log::Level::Debug) {
            log::log!(Level::Debug, $txt);
        }
    };
}
