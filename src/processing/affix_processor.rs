use log::{debug, Level, log_enabled};
use crate::model::config::{AFFIX_FIELDS, ConfigInput, InputAffix};
use crate::model::playlist::{FetchedPlaylist, FieldAccessor, PlaylistItem};
use crate::valid_property;

type AffixProcessor<'a> = Box<dyn Fn(&mut PlaylistItem) + 'a>;

fn create_affix_processor(affix: &InputAffix, is_prefix: bool) -> AffixProcessor {
    Box::new(move |channel: &mut PlaylistItem| {
        let header = &mut channel.header.borrow_mut();
        let value = if let Some(field_value) = header.get_field(affix.field.as_str()) {
            if is_prefix {
                format!("{}{}", field_value.as_str(), &affix.value)
            } else {
                format!("{}{}", &affix.value, field_value.as_str())
            }
        } else {
            String::from(&affix.value)
        };
        if log_enabled!(Level::Debug) {
            debug!("Applying input {}:  {}={}",  if is_prefix {"prefix"} else {"suffix"},  &affix.field, &value);
        }
        header.set_field(&affix.field, value.as_str());
    })
}

fn validate_and_create_affix_processor(affix: Option<&InputAffix>, is_prefix: bool) -> Option<AffixProcessor> {
    if let Some(affix_def) = affix {
        if (valid_property!(&affix_def.field.as_str(), AFFIX_FIELDS) && !affix_def.value.is_empty()) {
            return Some(create_affix_processor(affix_def, is_prefix));
        }
    };
    None
}

fn get_affix_processor(input: &ConfigInput) -> Option<AffixProcessor> {
    if input.suffix.is_some() || input.prefix.is_some() {
        let processors: Vec<AffixProcessor> = vec![
            validate_and_create_affix_processor(input.prefix.as_ref(), true),
            validate_and_create_affix_processor(input.suffix.as_ref(), false)
        ].into_iter().flatten().collect();

        if !processors.is_empty() {
            let apply_affix: AffixProcessor = Box::new(move |channel: &mut PlaylistItem| {
                for x in &processors {
                    x(channel);
                }
            });
            return Some(apply_affix);
        }
    }
    None
}

pub fn apply_affixes(fetched_playlists: &mut [FetchedPlaylist]) {
    fetched_playlists.iter_mut().for_each(|fetched_playlist| {
        let FetchedPlaylist { input, playlist, epg: _ } = fetched_playlist;
        if let Some(affix_processor) = get_affix_processor(input) {
            playlist.iter_mut().for_each(|group| {
                group.channels.iter_mut().for_each(|channel| {
                    affix_processor(channel);
                });
            });
        }
    });
}
