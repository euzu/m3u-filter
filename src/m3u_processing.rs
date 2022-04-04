use std::io::Write;
use config::ConfigTarget;

use crate::{config, Config, get_playlist, m3u, utils};
use crate::config::ItemField::Group;
use crate::config::SortOrder::{Asc, Desc};
use crate::filter::ValueProvider;
use crate::m3u::PlaylistItem;

fn check_write(res: std::io::Result<usize>) -> Result<(), std::io::Error> {
    match res {
        Ok(_) => Ok(()),
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to write file")),
    }
}

pub(crate) fn write_m3u(playlist: &Vec<m3u::PlaylistGroup>, target: &config::ConfigTarget, cfg: &config::Config) -> Result<(), std::io::Error> {
    let mut new_playlist: Vec<m3u::PlaylistGroup> = Vec::new();
    for g in playlist {
        let mut grp = g.clone();
        if target.rename.len() > 0 {
            for r in &target.rename {
                match r.field {
                    Group => {
                        let cap = r.re.as_ref().unwrap().replace_all(&grp.title, &r.new_name);
                        grp.title = cap.into_owned();
                    }
                    _ => {}
                }
            }
        }
        new_playlist.push(grp);
    }

    if let Some(sort) = &target.sort {
        new_playlist.sort_by(|a, b| {
            let ordering = a.title.partial_cmp(&b.title).unwrap();
            match sort.order {
                Asc => ordering,
                Desc => ordering.reverse()
            }
        });
    }
    match utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&target.filename))) {
        Some(path) => {
            let mut file = match std::fs::File::create(&path) {
                Ok(file) => file,
                Err(e) => {
                    println!("cant open file: {:?}", &path);
                    return Err(e);
                }
            };

            match check_write(file.write(b"#EXTM3U\n")) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
            for pg in &new_playlist {
                for pli in &pg.channels {
                    if is_valid(&pli, &target) {
                        let content = exec_rename(&pli, &target.rename).map_or_else(|| pli.to_m3u(&target.options), |p| p.to_m3u(&target.options));
                        match check_write(file.write(content.as_bytes())) {
                            Ok(_) => (),
                            Err(e) => return Err(e),
                        }
                        match check_write(file.write(b"\n")) {
                            Ok(_) => (),
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        }
        None => (),
    }
    Ok(())
}

fn get_field_value<'a>(pli: &'a m3u::PlaylistItem, field: &config::ItemField) -> &'a str {
    let value = match field {
        config::ItemField::Group => pli.header.group.as_str(),
        config::ItemField::Name => pli.header.name.as_str(),
        config::ItemField::Title => pli.header.title.as_str(),
    };
    value
}

fn set_field_value(pli: &mut m3u::PlaylistItem, field: &config::ItemField, value: String) -> () {
    let header = &mut pli.header;
    match field {
        config::ItemField::Group => header.group = value,
        config::ItemField::Name => header.name = value,
        config::ItemField::Title => header.title = value,
    };
}

fn is_valid(pli: &m3u::PlaylistItem, target: &ConfigTarget) -> bool {
    let provider = ValueProvider { pli };
    return target.filter(&provider);
}

fn exec_rename(pli: &m3u::PlaylistItem, rename: &Vec<config::ConfigRename>) -> Option<PlaylistItem> {
    if rename.len() > 0 {
        let mut result = pli.clone();
        for r in rename {
            let value = get_field_value(&result, &r.field);
            let cap = r.re.as_ref().unwrap().replace_all(value, &r.new_name);
            let value = cap.into_owned();
            set_field_value(&mut result, &r.field, value);
        }
        return Some(result);
    }
    None
}

pub fn process_targets(cfg: &Config, verbose: bool) {
    for source in cfg.sources.iter() {
        let url_str = source.input.url.as_str();
        let persist_file: Option<std::path::PathBuf> =
            if source.input.persist.is_empty() { None } else { utils::prepare_persist_path(source.input.persist.as_str()) };
        let file_path = utils::get_file_path(&cfg.working_dir, persist_file);
        if verbose { println!("persist file: {:?}", &file_path); }

        let result = get_playlist(&cfg.working_dir, url_str, file_path);
        match &result {
            Some(playlist) => {
                for target in source.targets.iter() {
                    match write_m3u(playlist, target, &cfg) {
                        Ok(_) => (),
                        Err(e) => println!("Failed to write file: {}", e)
                    }
                }
            }
            None => ()
        }
    }
}
