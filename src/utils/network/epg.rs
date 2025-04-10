use std::path::PathBuf;
use std::sync::Arc;
use log::debug;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigInput, EpgUrl};
use crate::model::xmltv::TVGuide;
use crate::repository::storage::{short_hash};
use crate::utils::file::file_utils::prepare_file_path;
use crate::utils::network::request;
use crate::utils::file::file_utils;


async fn download_epg_file(url: &str, client: &Arc<reqwest::Client>, input: &ConfigInput, working_dir: &str) -> Result<PathBuf, M3uFilterError> {
    debug!("Getting epg file path for url: {url}");
    let file_prefix = short_hash(url);
    let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "")
        .map(|path| file_utils::add_prefix_to_filename(&path, format!("{file_prefix}_epg_").as_str(), Some("xml")));

    request::get_input_text_content_as_file(Arc::clone(client), input, working_dir, url, persist_file_path).await
}

pub async fn get_xmltv(client: Arc<reqwest::Client>, _cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(urls) => {
            let mut errors = vec![];
            let mut file_paths = vec![];

            for url in match urls {
                EpgUrl::Single(value) => vec![value.to_string()],
                EpgUrl::Multi(value) => value.clone(),
            } {
                match download_epg_file(&url, &client, input, working_dir).await {
                    Ok(file_path) => {
                        file_paths.push(file_path);
                    }
                    Err(err) => {
                        errors.push(err);
                    }
                }
            }

            if file_paths.is_empty() {
                (None, errors)
            } else {
                (Some(TVGuide { file_paths }), errors)
            }
        }
    }
}