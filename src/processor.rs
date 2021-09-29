use std::fs::File;
use std::io::Write;

use crate::{config, m3u};
use crate::m3u::PlaylistItem;

pub fn write_m3u(playlist: &Vec<m3u::PlaylistGroup>, cfg: &config::Config) {
    for t in &cfg.targets {
        let mut file = match File::create(&t.filename) {
            Ok(file) => file,
            Err(_) => {
                println!("cant open file: {}", t.filename);
                std::process::exit(1);
            }
        };
        file.write(b"#EXTM3U\n").expect("Unable to write file");
        for pg in playlist {
            for pli in &pg.channels {
                if is_valid(&pli, &t.filter) {
                    let content = exec_rename(&pli, &t.rename).map_or_else(|| pli.to_m3u(), |p| p.to_m3u());
                    file.write(content.as_bytes()).expect("Unable to write file");
                    file.write(b"\n").expect("Unable to write file");
                }
            }
        }
    }
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
