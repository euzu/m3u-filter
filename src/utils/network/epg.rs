use std::sync::Arc;
use log::debug;
use std::fs::File;
use std::io::BufReader;

use crate::model::xmltv::{Epg, XmlTag};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput};
use crate::model::xmltv::TVGuide;
use crate::utils::file::file_utils::prepare_file_path;
use crate::utils::network::request;
use crate::utils::file::file_utils;

pub async fn get_xmltv(client: Arc<reqwest::Client>, _cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(url) => {
            debug!("Getting epg file path for url: {}", url);
            let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "")
                .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")));

            match request::get_input_text_content_as_file(client, input, working_dir, url, persist_file_path).await {
                Ok(file) => {
                    (Some(TVGuide { file }), vec![])
                }
                Err(err) => (None, vec![err])
            }
        }
    }
}


pub fn parse_epg(path: &std::path::Path) -> Result<Epg, M3uFilterError> {
    let file = File::open(path).map_err(|e| {
        M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to parse EPG: {}", e))
        //M3uFilterError::new(M3uFilterErrorKind::IOError, format!("Failed to open EPG file: {}", e))
    })?;
    let reader = BufReader::new(file);

    XmlTag::parse_root(reader).map_err(|e| {
        M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to parse XMLTV: {}", e))
        //M3uFilterError::new(M3uFilterErrorKind::ParseError, format!("Failed to parse XMLTV: {}", e))
    })
}
