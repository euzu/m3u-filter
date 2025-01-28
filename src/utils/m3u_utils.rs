use std::sync::Arc;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::PlaylistGroup;
use crate::processing::m3u_parser;
use crate::utils::download::prepare_file_path;
use crate::utils::request_utils;

pub async fn get_m3u_playlist(client: Arc<reqwest::Client>, cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let url = input.url.clone();
    let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "");
    match request_utils::get_input_text_content(client, input, working_dir, &url, persist_file_path).await {
        Ok(text) => {
            (m3u_parser::parse_m3u(cfg, input, text.lines()), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}
