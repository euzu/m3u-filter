use std::fs::File;
use std::path::{Path, PathBuf};

use actix_web::{HttpRequest, HttpResponse, web, http::header};
use log::{debug, info};
use quick_xml::{Reader, Writer};
use url::{ParseError};
use flate2::write::GzEncoder;
use flate2::Compression;
use quick_xml::events::{BytesStart, Event};
use std::io::{BufReader};
use chrono::{Duration, NaiveDateTime, TimeDelta};

use crate::api::api_model::{AppState, UserApiRequest};
use crate::api::api_utils::{get_user_target, serve_file};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::m3u_get_epg_file_path;
use crate::repository::storage::get_target_storage_path;
use crate::repository::xtream_repository::{xtream_get_epg_file_path, xtream_get_storage_path};
use crate::utils::{file_utils, request_utils};

fn time_correct(date_time: &str, correction: &TimeDelta) -> String {
    // Split the dateTime string into date and time parts
    let date_time_split: Vec<&str> = date_time.split(' ').collect();
    if date_time_split.len() != 2 {
        return date_time.to_string();
    }

    // Parse the datetime string
    match NaiveDateTime::parse_from_str(date_time_split[0], "%Y%m%d%H%M%S") {
        Ok(native_dt) => {
            let corrected_dt = native_dt + *correction;
            // Format the corrected datetime back to string
            let formatted_dt = corrected_dt.format("%Y%m%d%H%M%S").to_string();
            let result = format!("{} {}", formatted_dt, date_time_split[1]);
            result
        }
        Err(_) => date_time.to_string()
    }
}

fn get_epg_path_for_target_of_type(target_name: &str, file_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(epg_path) = file_path {
        if file_utils::path_exists(&epg_path) {
            return Some(epg_path);
        }
        info!("Cant find epg file for {target_name} target: {}", epg_path.to_str().unwrap_or("?"));
    }
    None
}

fn get_epg_path_for_target(config: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    // TODO if we share the same virtual_id for epg, can we store an epg file for the target ?
    for output in &target.output {
        match output.target {
            TargetType::M3u => {
                if let Some(target_path) = get_target_storage_path(config, &target.name) {
                    return get_epg_path_for_target_of_type(&target.name, m3u_get_epg_file_path(&target_path));
                }
            }
            TargetType::Xtream => {
                if let Some(storage_path) = xtream_get_storage_path(config, &target.name) {
                    return get_epg_path_for_target_of_type(&target.name, Some(xtream_get_epg_file_path(&storage_path)));
                }
            }
            TargetType::Strm => {}
        }
    }
    None
}

fn parse_timeshift(time_shift: Option<&String>) -> Option<i32> {
    match time_shift {
        None => None,
        Some(offset) => {
            let sign_factor = if offset.starts_with('-') { -1 } else { 1 };
            let offset = offset.trim_start_matches(&['-', '+'][..]); // Remove the sign for parsing

            let total_minutes = if offset.contains(':') {
                // Handle the case with hours and minutes (e.g., "-2:30", "1:45", "+0:15", ":30")
                let parts: Vec<&str> = offset.split(':').collect();

                let hours: i32 = if parts[0].is_empty() {
                    0 // Treat empty hour part as 0 hours
                } else {
                    parts[0].parse().unwrap_or(0)
                };

                let minutes: i32 = if parts.len() > 1 {
                    parts[1].parse().unwrap_or(0)
                } else {
                    0
                };

                // Convert hours to minutes and add the minute part
                hours * 60 + minutes
            } else {
                // Handle single number case (e.g., "2" or "+2")
                let num: i32 = offset.parse().unwrap_or(0);
                num * 60
            };

            if total_minutes > 0 {
                Some(sign_factor * total_minutes)
            } else {
                None
            }
        }
    }
}

async fn serve_epg(epg_path: &Path, req: &HttpRequest, user: &ProxyUserCredentials) -> HttpResponse {
    match File::open(epg_path) {
        Ok(epg_file) => {
            match parse_timeshift(user.epg_timeshift.as_ref()) {
                None => serve_file(epg_path, req, mime::TEXT_XML).await,
                Some(duration) => {
                    serve_epg_with_timeshift(epg_file, duration)
                }
            }
        }
        Err(_) => {
            HttpResponse::NoContent().finish()
        }
    }
}

fn serve_epg_with_timeshift(epg_file: File, offset_minutes: i32) -> HttpResponse {
    let reader = BufReader::new(epg_file);
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut xml_reader = Reader::from_reader(reader);
    let mut xml_writer = Writer::new(encoder);
    let mut buf = Vec::new();
    let duration = Duration::minutes(i64::from(offset_minutes));

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"programme" => {
                // Modify the attributes
                let mut elem = BytesStart::from(e.name());
                for attr in e.attributes() {
                    match attr {
                        Ok(attr) if attr.key.as_ref() == b"start" => {
                            let start_value = attr.decode_and_unescape_value(xml_reader.decoder())
                                .expect("Failed to decode start attribute");
                            // Modify the start attribute value as needed
                            elem.push_attribute(("start", time_correct(&start_value, &duration).as_str()));
                        }
                        Ok(attr) if attr.key.as_ref() == b"stop" => {
                            let stop_value = attr.decode_and_unescape_value(xml_reader.decoder())
                                .expect("Failed to decode stop attribute");
                            // Modify the stop attribute value as needed
                            elem.push_attribute(("stop", time_correct(&stop_value, &duration).as_str()));
                        }
                        Ok(attr) => {
                            // Copy any other attributes as they are
                            elem.push_attribute(attr);
                        }
                        Err(e) => {
                            println!("Error parsing attribute: {e}");
                        }
                    }
                }

                // Write the modified start event
                xml_writer.write_event(Event::Start(elem)).expect("Failed to write event");
            }
            Ok(Event::Eof) => break, // End of file
            Ok(event) => {
                // Write any other event as is
                xml_writer.write_event(event).expect("Failed to write event");
            }
            Err(e) => {
                println!("Error: {e}");
                break;
            }
        }

        buf.clear();
    }

    let compressed_data = xml_writer.into_inner().finish().unwrap();
    HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((header::CONTENT_ENCODING, "gzip")) // Set Content-Encoding header
        .body(compressed_data)
}

async fn xmltv_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some((user, target)) = get_user_target(&api_req, &app_state) {
        match get_epg_path_for_target(&app_state.config, target) {
            None => {
                // No epg configured, we try to figure out an epg url and redirect.
                // No processing or timeshift 
                if let Some(value) = get_xmltv_raw_epg(&app_state.config, &user, &target.name).await {
                    return value;
                }
            }
            Some(epg_path) => return serve_epg(&epg_path, &req, &user).await
        }
    }
    HttpResponse::Ok().content_type(mime::TEXT_XML).body(
        r#"<?xml version="1.0" encoding="utf-8" ?><!DOCTYPE tv SYSTEM "xmltv.dtd"><tv generator-info-name="Xtream Codes" generator-info-url=""></tv>"#)
}

fn get_xmltv_epg_url(input: &ConfigInput) -> Result<String, ParseError> {
    let epg_url = input.epg_url.as_ref().map_or(String::new(), std::borrow::ToOwned::to_owned);
    if epg_url.is_empty() {
        if let Some(user_info) = input.get_user_info() {
            let url = user_info.base_url.as_str();
            let username = user_info.username.as_str();
            let password = user_info.password.as_str();
            Ok(format!("{url}/xmltv.php?username={username}&password={password}"))
        } else {
            Err(ParseError::EmptyHost)
        }
    } else {
        Ok(epg_url)
    }
}

async fn get_xmltv_raw_epg(config: &Config, user: &ProxyUserCredentials, target_name: &str) -> Option<HttpResponse> {
    // If no epg_url is provided for input, we did not process the xmltv for our channels.
    // We are now delivering the original untouched xmltv.
    // If you want to use xmltv then provide the url in the config to filter unnecessary content.
    // If you have multiple xtream sources, no response because of mapped ids
    // if you want epg for multi xtream input, then provide  epg_url.
    if let Some(inputs) = config.get_inputs_for_target(target_name) {
        if inputs.len() == 1 {
            if let Some(&input) = inputs.first() {
                if let Ok(url) = get_xmltv_epg_url(input) {
                    if user.proxy == ProxyType::Redirect {
                        debug!("Redirecting epg request to {}", url.as_str());
                        return Some(HttpResponse::Found().insert_header(("Location", url.as_str())).finish());
                    }
                    match request_utils::download_text_content(input, url.as_str(), None).await {
                        Ok(content) => return Some(HttpResponse::Ok().content_type(mime::TEXT_XML).body(content)),
                        Err(err) => {
                            debug!("Could not generate epg url for {target_name} {err}");
                        }
                    }
                } else {
                    debug!("Could not generate epg url for {target_name}");
                }
            }
        } else {
            debug!("No epg_url is provided for target {target_name}, multi input requires epg_url");
        }
    }
    None
}

pub(crate) fn xmltv_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/xmltv.php").route(web::get().to(xmltv_api)))
        .service(web::resource("/epg").route(web::get().to(xmltv_api)));
}
