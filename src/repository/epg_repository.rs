use std::fs::File;
use std::io::Write;
use std::path::{Path};
use log::debug;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, TargetOutput};
use crate::model::model_config::TargetType;
use crate::model::xmltv::TVGuide;
use crate::repository::m3u_repository::{get_m3u_epg_file_path};
use crate::repository::xtream_repository::{get_xtream_epg_file_path, get_xtream_storage_path};

fn write_epg_file(target: &ConfigTarget, xml_content: &String, path: &Path) -> Result<(), M3uFilterError> {
    match File::create(path) {
        Ok(mut epg_file) => {
            match epg_file.write_all("<?xml version=\"1.0\" encoding=\"utf-8\" ?><!DOCTYPE tv SYSTEM \"xmltv.dtd\">".as_bytes()) {
                Ok(_) => {}
                Err(_) => {}
            }
            match epg_file.write_all(xml_content.as_bytes()) {
                Ok(_) => {
                    debug!("Epg for target {} written to {}", target.name, path.to_str().unwrap_or("?"))
                }
                Err(err) => return Err(M3uFilterError::new(
                    M3uFilterErrorKind::Notify, format!("failed to write epg: {} - {}", path.to_str().unwrap_or("?"), err))),
            }
        }
        Err(err) => return Err(M3uFilterError::new(
            M3uFilterErrorKind::Notify, format!("failed to write epg: {} - {}", path.to_str().unwrap_or("?"), err))),
    }
    Ok(())
}

pub(crate) fn write_epg(target: &ConfigTarget, cfg: &Config, tv_guide: &Option<TVGuide>, output: &TargetOutput) -> Result<(), M3uFilterError> {
    if let Some(epg_data) = tv_guide {
        match quick_xml::se::to_string(&epg_data) {
            Ok(xml_content) => {
                debug!("serde to xml ok ");
                match &output.target {
                    TargetType::M3u => {
                        if output.filename.is_none() {
                            return Err(M3uFilterError::new(
                                M3uFilterErrorKind::Notify,
                                format!("write epg for target {} failed: No filename set", target.name)));
                        }
                        if let Some(path) = get_m3u_epg_file_path(cfg, &output.filename) {
                            debug!("writing m3u epg to {}", path.to_str().unwrap_or("?"));
                            write_epg_file(target, &xml_content, &path)?
                        }
                    }
                    TargetType::Xtream => {
                        match get_xtream_storage_path(cfg, &target.name) {
                            Some(path) => {
                                let epg_path = get_xtream_epg_file_path(&path);
                                debug!("writing xtream epg to {}", epg_path.to_str().unwrap_or("?"));
                                write_epg_file(target, &xml_content, &epg_path)?
                            }
                            None => return Err(M3uFilterError::new(
                                M3uFilterErrorKind::Notify,
                                format!("failed to serialize epg for target: {}, storage path not found", target.name))),
                        }
                    }
                    TargetType::Strm => {}
                }
            },
            Err(err) => return Err(M3uFilterError::new(
                M3uFilterErrorKind::Notify, format!("failed to serialize epg for target: {} - {}", target.name, err))),
        }
    }
    Ok(())
}
