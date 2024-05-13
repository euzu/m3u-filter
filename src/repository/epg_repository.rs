use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path};
use log::{debug, log_enabled, Level};
use quick_xml::{Writer};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, TargetOutput};
use crate::model::config::TargetType;
use crate::model::xmltv::{Epg};
use crate::repository::m3u_repository::{m3u_get_epg_file_path};
use crate::repository::xtream_repository::{xtream_get_epg_file_path, xtream_get_storage_path};

fn epg_write_file(target: &ConfigTarget, epg: &Epg, path: &Path) -> Result<(), M3uFilterError> {
    let mut writer = Writer::new(Cursor::new(vec![]));
    match epg.write_to(&mut writer) {
        Ok(()) => {
            let result = writer.into_inner().into_inner();
            match File::create(path) {
                Ok(mut epg_file) => {
                    match epg_file.write_all("<?xml version=\"1.0\" encoding=\"utf-8\" ?><!DOCTYPE tv SYSTEM \"xmltv.dtd\">".as_bytes()) {
                        Ok(()) => {}
                        Err(err) => return Err(M3uFilterError::new(
                            M3uFilterErrorKind::Notify, format!("failed to write epg: {} - {}", path.to_str().unwrap_or("?"), err))),
                    }
                    match epg_file.write_all(&result) {
                        Ok(()) => {
                            if log_enabled!(Level::Debug) {
                                debug!("Epg for target {} written to {}", target.name, path.to_str().unwrap_or("?"));
                            }
                        }
                        Err(err) => return Err(M3uFilterError::new(
                            M3uFilterErrorKind::Notify, format!("failed to write epg: {} - {}", path.to_str().unwrap_or("?"), err))),
                    }
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

pub(crate) fn epg_write(target: &ConfigTarget, cfg: &Config, epg: Option<&Epg>, output: &TargetOutput) -> Result<(), M3uFilterError> {
    if let Some(epg_data) = epg {
        match &output.target {
            TargetType::M3u => {
                if output.filename.is_none() {
                    return Err(M3uFilterError::new(
                        M3uFilterErrorKind::Notify,
                        format!("write epg for target {} failed: No filename set", target.name)));
                }
                if let Some(path) = m3u_get_epg_file_path(cfg, target) {
                    if log_enabled!(Level::Debug) {
                        debug!("writing m3u epg to {}", path.to_str().unwrap_or("?"));
                    }
                    epg_write_file(target, epg_data, &path)?;
                }
            }
            TargetType::Xtream => {
                match xtream_get_storage_path(cfg, &target.name) {
                    Some(path) => {
                        let epg_path = xtream_get_epg_file_path(&path);
                        if log_enabled!(Level::Debug) {
                            debug!("writing xtream epg to {}", epg_path.to_str().unwrap_or("?"));
                        }
                        epg_write_file(target, epg_data, &epg_path)?;
                    }
                    None => return Err(M3uFilterError::new(
                        M3uFilterErrorKind::Notify,
                        format!("failed to serialize epg for target: {}, storage path not found", target.name))),
                }
            }
            TargetType::Strm => {}
        }
    }
    Ok(())
}
