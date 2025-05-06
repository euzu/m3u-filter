use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::IpCheckConfig;
use regex::Regex;
use reqwest::Client;
use std::sync::Arc;

async fn fetch_ip(client: &Arc<Client>, url: &str, regex: Option<&Regex>) -> Result<String, M3uFilterError> {
    let response = client.get(url).send().await
        .map_err(|e| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Failed to request {url}: {e}")))?;

    let text = response.text().await
        .map_err(|e| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Failed to read response: {e}")))?;

    if let Some(re) = regex {
        return if let Some(caps) = re.captures(&text) {
            if let Some(m) = caps.get(1) {
                Ok(m.as_str().to_string())
            } else {
                Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Regex matched but no group found".to_string()))
            }
        } else {
            Err(M3uFilterError::new(M3uFilterErrorKind::Info, "Regex did not match".to_string()))
        };
    }

    Ok(text.trim().to_string())
}

/// Fetch both IPs from a shared URL (if both regex patterns are available)
async fn fetch_combined_ips(client: &Arc<Client>, config: &IpCheckConfig, url: &str) -> (Option<String>, Option<String>) {
    let response = client.get(url).send().await.ok();
    let text = match response {
        Some(r) => r.text().await.ok(),
        None => None,
    };

    if let Some(body) = text {
        let ipv4 = config
            .t_pattern_ipv4
            .as_ref()
            .and_then(|re| re.captures(&body))
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()));

        let ipv6 = config
            .t_pattern_ipv6
            .as_ref()
            .and_then(|re| re.captures(&body))
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()));

        (ipv4, ipv6)
    } else {
        (None, None)
    }
}

/// Fetch both IPv4 and IPv6 addresses, using separate or combined URL(s)
pub async fn get_ips(client: &Arc<Client>, config: &IpCheckConfig) -> Result<(Option<String>, Option<String>), M3uFilterError> {
    match (&config.url_ipv4, &config.url_ipv6, &config.url) {
        // Both dedicated URLs provided
        (Some(url_v4), Some(url_v6), _) => {
            let (ipv4, ipv6) = tokio::join!(
                    fetch_ip(client, url_v4, config.t_pattern_ipv4.as_ref()),
                    fetch_ip(client, url_v6, config.t_pattern_ipv6.as_ref())
                );
            Ok((ipv4.ok(), ipv6.ok()))
        }

        // Only one combined URL provided
        (_, _, Some(shared_url)) => {
            let result = fetch_combined_ips(client, config, shared_url).await;
            Ok(result)
        }

        // Only one dedicated URL
        (Some(url_v4), None, _) => {
            let ipv4 = fetch_ip(client, url_v4, config.t_pattern_ipv4.as_ref()).await.ok();
            Ok((ipv4, None))
        }
        (None, Some(url_v6), _) => {
            let ipv6 = fetch_ip(client, url_v6, config.t_pattern_ipv6.as_ref()).await.ok();
            Ok((None, ipv6))
        }

        // No URLs given
        _ => Err(M3uFilterError::new(M3uFilterErrorKind::Info, "No valid IP-check URLs provided".to_owned())),
    }
}