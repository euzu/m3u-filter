use regex::Regex;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::LazyLock;


pub const USER_FILE: &str = "user.txt";
pub const CONFIG_PATH: &str = "config";
pub const CONFIG_FILE: &str = "config.yml";
pub const SOURCE_FILE: &str = "source.yml";
pub const MAPPING_FILE: &str = "mapping.yml";
pub const API_PROXY_FILE: &str = "api-proxy.yml";


pub const ENCODING_GZIP: &str = "gzip";
pub const ENCODING_DEFLATE: &str = "deflate";


pub const HLS_EXT: &str = ".m3u8";
pub const DASH_EXT: &str = ".mpd";

pub const HLS_PREFIX: &str = "hls";

pub const HLS_EXT_QUERY: &str = ".m3u8?";
pub const HLS_EXT_FRAGMENT: &str = ".m3u8#";
pub const DASH_EXT_QUERY: &str = ".mpd?";
pub const DASH_EXT_FRAGMENT: &str = ".mpd#";

pub const FILENAME_TRIM_PATTERNS: &[char] = &['.', '-', '_'];

pub const MEDIA_STREAM_HEADERS: &[&str] = &["accept", "content-type", "content-length", "connection",
    "accept-ranges", "content-range", "vary", "transfer-encoding", "access-control-allow-origin",
    "access-control-allow-credentials", "icy-metadata"];

pub struct KodiStyle {
    pub year: Regex,
    pub season: Regex,
    pub episode: Regex,
    pub whitespace: Regex,
    pub alphanumeric: Regex,
}

pub struct Constants {
    pub re_username: Regex,
    pub re_password: Regex,
    pub re_token: Regex,
    pub re_stream_url: Regex,
    pub re_url: Regex,
    pub re_base_href: Regex,
    pub re_env_var: Regex,
    pub re_memory_usage: Regex,
    pub re_epg_normalize: Regex,
    pub re_template_var: Regex,
    pub re_template_tag: Regex,
    pub re_template_attribute: Regex,
    pub re_filename: Regex,
    pub re_remove_filename_ending: Regex,
    pub re_whitespace: Regex,
    pub re_hls_uri: Regex,
    pub sanitize: AtomicBool,
    pub kodi_style: KodiStyle,
    pub country_codes: HashSet<&'static str>,
}

pub static CONSTANTS: LazyLock<Constants> = LazyLock::new(||
    Constants {
        re_username: Regex::new(r"(username=)[^&]*").unwrap(),
        re_password: Regex::new(r"(password=)[^&]*").unwrap(),
        re_token: Regex::new(r"(token=)[^&]*").unwrap(),
        re_stream_url: Regex::new(r"(.*://).*/(live|video|movie|series|m3u-stream|resource)/\w+/\w+").unwrap(),
        re_url: Regex::new(r"(.*://).*?/(.*)").unwrap(),
        re_base_href: Regex::new(r#"(href|src)="/([^"]*)""#).unwrap(),
        re_env_var: Regex::new(r"\$\{env:(?P<var>[a-zA-Z_][a-zA-Z0-9_]*)}").unwrap(),
        re_memory_usage: Regex::new(r"VmRSS:\s+(\d+) kB").unwrap(),
        re_epg_normalize: Regex::new(r"[^a-zA-Z0-9\-]").unwrap(),
        re_template_var: Regex::new("!(.*?)!").unwrap(),
        re_template_tag: Regex::new("<tag:(.*?)>").unwrap(),
        re_template_attribute: Regex::new("<(.*?)>").unwrap(),
        re_filename: Regex::new(r"[^A-Za-z0-9_.-]").unwrap(),
        re_remove_filename_ending: Regex::new(r"[_.\s-]$").unwrap(),
        re_whitespace: Regex::new(r"\s+").unwrap(),
        re_hls_uri: Regex::new(r#"URI="([^"]+)""#).unwrap(),

        sanitize: AtomicBool::new(true),
        kodi_style: KodiStyle {
            season: Regex::new(r"[Ss]\d{1,2}").unwrap(),
            episode: Regex::new(r"[Ee]\d{1,2}").unwrap(),
            year: Regex::new(r"\d{4}").unwrap(),
            whitespace: Regex::new(r"\s+").unwrap(),
            alphanumeric: Regex::new(r"[^\w\s]").unwrap(),
        },
        country_codes: vec![
            "af", "al", "dz", "ad", "ao", "ag", "ar", "am", "au", "at", "az", "bs", "bh", "bd", "bb", "by",
            "be", "bz", "bj", "bt", "bo", "ba", "bw", "br", "bn", "bg", "bf", "bi", "cv", "kh", "cm", "ca",
            "cf", "td", "cl", "cn", "co", "km", "cg", "cr", "hr", "cu", "cy", "cz", "cd", "dk", "dj", "dm",
            "do", "tl", "ec", "eg", "sv", "gq", "er", "ee", "sz", "et", "fj", "fi", "fr", "ga", "gm", "ge",
            "de", "gh", "gr", "gd", "gt", "gn", "gw", "gy", "ht", "hn", "hu", "is", "in", "id", "ir", "iq",
            "ie", "il", "it", "ci", "jm", "jp", "jo", "kz", "ke", "ki", "kp", "kr", "kw", "kg", "la", "lv",
            "lb", "ls", "lr", "ly", "li", "lt", "lu", "mg", "mw", "my", "mv", "ml", "mt", "mh", "mr", "mu",
            "mx", "fm", "md", "mc", "mn", "me", "ma", "mz", "mm", "na", "nr", "np", "nl", "nz", "ni", "ne",
            "ng", "mk", "no", "om", "pk", "pw", "pa", "pg", "py", "pe", "ph", "pl", "pt", "qa", "ro", "ru",
            "rw", "kn", "lc", "vc", "ws", "sm", "st", "sa", "sn", "rs", "sc", "sl", "sg", "sk", "si", "sb",
            "so", "za", "ss", "es", "lk", "sd", "sr", "se", "ch", "sy", "tw", "tj", "tz", "th", "tg", "to",
            "tt", "tn", "tr", "tm", "tv", "ug", "ua", "ae", "gb", "us", "uy", "uz", "vu", "va", "ve", "vn",
            "ye", "zm", "zw",
        ].into_iter().collect::<HashSet<&str>>(),
    }
);
