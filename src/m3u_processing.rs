use std::io::{ Write};

use crate::{config, m3u, utils};
use crate::m3u::PlaylistItem;

fn check_write(res: std::io::Result<usize>) -> Result<(), std::io::Error> {
    match res {
        Ok(_) => Ok(()),
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to write file")),
    }
}

pub(crate) fn write_m3u(playlist: &Vec<m3u::PlaylistGroup>, cfg: &config::Config) -> Result<(), std::io::Error> {
    for t in &cfg.targets {
        match utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&t.filename))) {
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
                for pg in playlist {
                    for pli in &pg.channels {
                        if is_valid(&pli, &t.filter) {
                            let content = exec_rename(&pli, &t.rename).map_or_else(|| pli.to_m3u(), |p| p.to_m3u());
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
            },
            None => (),
        }
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

fn is_valid(pli: &m3u::PlaylistItem, filter: &config::ConfigFilters) -> bool {
    let mut matched = false;
    for r in &filter.rules {
        let value = get_field_value(pli, &r.field);
        matched = r.re.as_ref().unwrap().is_match(value);
        if matched {
            break;
        }
    }
    return if filter.is_include() {
        matched
    } else {
        !matched
    };
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
