use std::sync::Arc;
use crate::tuliprox_error::TuliProxError;
use crate::model::{Config, ConfigInput};
use crate::model::PlaylistGroup;
use crate::processing::parser::m3u;
use crate::utils::file_utils::prepare_file_path;
use crate::utils::request;

pub async fn get_m3u_playlist(client: Arc<reqwest::Client>, cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<TuliProxError>) {
    let url = input.url.clone();
    let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "");
    match request::get_input_text_content(client, input, working_dir, &url, persist_file_path).await {
        Ok(text) => {
            (m3u::parse_m3u(cfg, input, text.lines()), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}
