use std::borrow::Cow;
use crate::utils::{file_utils};
use std::path::PathBuf;
use crate::debug_if_enabled;


pub fn prepare_file_path(persist: Option<&str>, working_dir: &str, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        persist.map(|persist_path| file_utils::prepare_persist_path(persist_path, action));
    if persist_file.is_some() {
        let file_path = file_utils::get_file_path(working_dir, persist_file);
        debug_if_enabled!("persist to file:  {}", file_path.as_ref().map_or(Cow::from("?"), |p| p.to_string_lossy()));
        file_path
    } else {
        None
    }
}

