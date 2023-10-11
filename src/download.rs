use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use log::debug;
use crate::{m3u_parser, utils, xtream_parser};
use crate::model::config::{Config, ConfigInput};
use crate::model::model_m3u::{PlaylistGroup, XtreamCluster};

fn prepare_file_path(input: &ConfigInput, working_dir: &String, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        match &input.persist {
            Some(persist_path) => utils::prepare_persist_path(persist_path.as_str(), action),
            _ => None
        };
    if persist_file.is_some() {
        let file_path = utils::get_file_path(working_dir, persist_file);
        debug!("persist to file:  {:?}", match &file_path {
            Some(fp) => fp.display().to_string(),
            _ => "".to_string()
        });
        file_path
    } else {
        None
    }
}

pub(crate) fn get_m3u_playlist(cfg: &Config, input: &ConfigInput, working_dir: &String) -> Option<Vec<PlaylistGroup>> {
    let url = input.url.as_str();
    let file_path = prepare_file_path(input, working_dir, "");
    let lines: Option<Vec<String>> = utils::get_input_content(working_dir, url, file_path);
    lines.map(|l| m3u_parser::parse_m3u(cfg, &l))
}

/*
get_live_categories, get_vod_categories, get_live_categories ->
[
  {
    "category_id": "225",
    "category_name": "Public Channels",
    "parent_id": 0
  },
  {
    "category_id": "240",
    "category_name": "Public Movies",
    "parent_id": 0
  }
]

get_series -> [
  {
    "backdrop_path": [
      "https://image.tmdb.org/.....jpg"
    ],
    "cast": "Barakuda Marakuda",
    "category_id": "72",
    "category_ids": [
      72
    ],
    "cover": "https://image.tmdb.org/....jpg",
    "director": null,
    "episode_run_time": "0",
    "genre": "Science Fiction",
    "last_modified": "1688398196",
    "name": "Monsieur Barakuda (2023)",
    "num": 16,
    "plot": "The darkest hour in history, the future is volatile.",
    "rating": "5",
    "rating_5based": 2.5,
    "releaseDate": "2023-06-05",
    "release_date": "2023-06-05",
    "series_id": 2192,
    "stream_type": "series",
    "title": "Monsieur Barakuda",
    "year": "2023",
    "youtube_trailer": null
  }
 ]

get_vod_streams ->
[
  {
    "added": "1603364032",
    "cast": "Pirle Palle, Milli Vanilla",
    "category_id": "195",
    "category_ids": [
      195
    ],
    "container_extension": "mkv",
    "custom_sid": "",
    "direct_source": "http://192.168.0.2:8080/play/jnmj-bubblegum",
    "director": "Joe Barlow",
    "episode_run_time": "123",
    "genre": "Crime, Drama, Mystery",
    "name": "Enola Holmes (2020)",
    "num": 1,
    "plot": "While searching for her missing mother, she goes crozy.",
    "rating": 7.6,
    "rating_5based": 3.8,
    "release_date": null,
    "stream_icon": "https://image.tmdb.org/....jpg",
    "stream_id": 95078,
    "stream_type": "movie",
    "title": "Search for a mother (2020)",
    "year": null,
    "youtube_trailer": "dd9Zf9sXlHk"
  }
]


get_live_streams ->
[
  {
    "added": "1602322663",
    "category_id": "125",
    "category_ids": [
      125
    ],
    "custom_sid": "",
    "direct_source": "http://192.168.0.2:8080/play/qlv9ZgTZdRFC8wct0n678YUIYlctwQg9ZBV",
    "epg_channel_id": "360.fr",
    "name": "FR | FR 1 SD",
    "num": 1,
    "stream_icon": "https://imagizer.imageshack.com/.....png",
    "stream_id": 88019,
    "stream_type": "live",
    "thumbnail": "",
    "tv_archive": 0,
    "tv_archive_duration": 0
  },
  {
    "added": "1586779266",
    "category_id": "148",
    "category_ids": [
      148
    ],
    "custom_sid": "",
    "direct_source": "http://192.168.0.2:8080/play/qlv9ZgTZdHGC8wct0n318YUIYlctwQg9ZBV",
    "epg_channel_id": null,
    "name": "FR | RADIO VIVA LA FRANCE",
    "num": 9119,
    "stream_icon": "https://imagizer.imageshack.com/.....png",
    "stream_id": 27569,
    "stream_type": "radio_streams",
    "thumbnail": "",
    "tv_archive": 0,
    "tv_archive_duration": 0
  }
]

 */
const ACTIONS: [(XtreamCluster, &str, &str); 3] = [
    (XtreamCluster::Live, "get_live_categories", "get_live_streams"),
    (XtreamCluster::Video, "get_vod_categories", "get_vod_streams"),
    (XtreamCluster::Series, "get_series_categories", "get_series")];

pub(crate) fn get_xtream_playlist(input: &ConfigInput, working_dir: &String) -> Option<Vec<PlaylistGroup>> {
    let mut playlist: Vec<PlaylistGroup> = Vec::new();
    let username = input.username.as_ref().unwrap().clone();
    let password = input.password.as_ref().unwrap().clone();
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);
    let stream_base_url = format!("{}/{}/{}", input.url, username, password);

    let category_id_cnt = AtomicI32::new(0);
    for (xtream_cluster, category, stream) in &ACTIONS {
        let category_url = format!("{}&action={}", base_url, category);
        let stream_url = format!("{}&action={}", base_url, stream);
        let category_file_path = prepare_file_path(input, working_dir, format!("{}_", category).as_str());
        let stream_file_path = prepare_file_path(input, working_dir, format!("{}_", stream).as_str());

        let category_content: Option<serde_json::Value> = utils::get_input_json_content(input, &category_url, category_file_path);
        let stream_content: Option<serde_json::Value> = utils::get_input_json_content(input, &stream_url, stream_file_path);
        let mut sub_playlist = xtream_parser::parse_xtream(&category_id_cnt, xtream_cluster, category_content, stream_content, &stream_base_url);
        while let Some(group) = sub_playlist.pop() {
            playlist.push(group);
        }
    }
    Some(playlist)
}
