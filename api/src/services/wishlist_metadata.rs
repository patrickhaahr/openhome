//! Non-persistent product URL assistance. Untrusted pages supply editable suggestions only.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use url::{Host, Url};

use super::wishlist::WishlistError;

const MAX_BYTES: usize = 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataRequest {
    pub owner_id: String,
    pub list_id: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataStatus {
    Ready,
    Partial,
    Unavailable,
}

/// A preview never claims that an item was stored. Save reviewed fields separately.
#[derive(Debug, Serialize)]
pub struct MetadataPreview {
    pub status: MetadataStatus,
    pub url: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
    pub price: Option<String>,
}

#[derive(Clone, Default)]
pub struct MetadataService {
    // Unit transport tests retain the production address policy, then connect vetted demo
    // hosts to controlled loopback responders. No runtime setting can bypass SSRF checks.
    #[cfg(test)]
    hosts: std::collections::HashMap<String, (Vec<SocketAddr>, SocketAddr)>,
    #[cfg(test)]
    timeout: Option<Duration>,
}

enum FetchError {
    Blocked,
    Unavailable,
}

fn blocked() -> WishlistError {
    WishlistError::Validation("Product URL must use HTTP(S) without credentials and a public destination; enter item details manually".into())
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(matches!(a, 0 | 10 | 127 | 224..=255)
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || (b == 0 && matches!(c, 0 | 2))))
                || (a == 192 && b == 88 && c == 99)
                || (a == 198 && (matches!(b, 18 | 19) || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let segments = ip.segments();
            // Only global unicast, excluding special-use, documentation and 6to4 ranges.
            (segments[0] & 0xe000) == 0x2000
                && !(segments[0] == 0x2001 && (segments[1] < 0x200 || segments[1] == 0xdb8))
                && segments[0] != 0x2002
                && !(segments[0] == 0x3fff && segments[1] < 0x1000)
        }
    }
}

fn public_url(value: &str) -> Result<Url, FetchError> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(FetchError::Blocked);
    }
    let mut url = Url::parse(value.trim()).map_err(|_| FetchError::Blocked)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(FetchError::Blocked);
    }
    match url.host() {
        Some(Host::Ipv4(ip)) if public_ip(IpAddr::V4(ip)) => (),
        Some(Host::Ipv6(ip)) if public_ip(IpAddr::V6(ip)) => (),
        Some(Host::Domain(host))
            if host.contains('.')
                && !host.ends_with('.')
                && ![
                    "localhost",
                    "local",
                    "internal",
                    "home",
                    "lan",
                    "test",
                    "invalid",
                    "example",
                    "onion",
                ]
                .iter()
                .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}"))) => {}
        _ => return Err(FetchError::Blocked),
    }
    url.set_fragment(None);
    Ok(url)
}

impl MetadataService {
    pub async fn preview(&self, value: &str) -> Result<MetadataPreview, WishlistError> {
        let url = public_url(value).map_err(|_| blocked())?;
        let timeout = TIMEOUT;
        #[cfg(test)]
        let timeout = self.timeout.unwrap_or(timeout);
        match tokio::time::timeout(timeout, self.fetch(url.clone())).await {
            Ok(Ok((final_url, html))) => Ok(extract(&final_url, &html)),
            Ok(Err(FetchError::Blocked)) => Err(blocked()),
            Ok(Err(FetchError::Unavailable)) | Err(_) => Ok(MetadataPreview {
                status: MetadataStatus::Unavailable,
                url: url.into(),
                title: None,
                image_url: None,
                price: None,
            }),
        }
    }

    async fn addresses(&self, url: &Url) -> Result<Vec<SocketAddr>, FetchError> {
        let host = url.host_str().ok_or(FetchError::Blocked)?;
        let port = url.port_or_known_default().ok_or(FetchError::Blocked)?;
        #[cfg(test)]
        if let Some((addresses, _)) = self.hosts.get(host) {
            return Ok(addresses.clone());
        }
        if let Some(ip) = match url.host() {
            Some(Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
            _ => None,
        } {
            return Ok(vec![SocketAddr::new(ip, port)]);
        }
        tokio::net::lookup_host((host, port))
            .await
            .map(Iterator::collect)
            .map_err(|_| FetchError::Unavailable)
    }

    async fn fetch(&self, mut url: Url) -> Result<(Url, String), FetchError> {
        for hop in 0..=MAX_REDIRECTS {
            let addresses = self.addresses(&url).await?;
            if addresses.is_empty() {
                return Err(FetchError::Unavailable);
            }
            if addresses.iter().any(|address| !public_ip(address.ip())) {
                return Err(FetchError::Blocked);
            }
            let host = url.host_str().ok_or(FetchError::Blocked)?;
            #[cfg(test)]
            let addresses = if let Some((_, connection)) = self.hosts.get(host) {
                vec![*connection]
            } else {
                addresses
            };
            // Pin the exact DNS answers we vetted: resolving again would allow DNS rebinding.
            // Disable environment proxies, cookies, automatic redirects and compression.
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(TIMEOUT)
                .connect_timeout(Duration::from_secs(3))
                .resolve_to_addrs(host, &addresses)
                .build()
                .map_err(|_| FetchError::Unavailable)?;
            let mut response = client
                .get(url.clone())
                .header("Accept", "text/html, application/xhtml+xml")
                .header("Accept-Encoding", "identity")
                .header("User-Agent", "OpenHome-Wishlist/1.0")
                .send()
                .await
                .map_err(|_| FetchError::Unavailable)?;
            if response.status().is_redirection() {
                if hop == MAX_REDIRECTS {
                    return Err(FetchError::Unavailable);
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(FetchError::Unavailable)?;
                let next = url.join(location).map_err(|_| FetchError::Blocked)?;
                url = public_url(next.as_str())?;
                continue;
            }
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .trim();
            let encoded = response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_some_and(|value| value != "identity");
            if !response.status().is_success()
                || !matches!(content_type, "text/html" | "application/xhtml+xml")
                || encoded
                || response
                    .content_length()
                    .is_some_and(|length| length > MAX_BYTES as u64)
            {
                return Err(FetchError::Unavailable);
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| FetchError::Unavailable)?
            {
                if chunk.len() > MAX_BYTES - body.len() {
                    return Err(FetchError::Unavailable);
                }
                body.extend_from_slice(&chunk);
            }
            return Ok((url, String::from_utf8_lossy(&body).into_owned()));
        }
        Err(FetchError::Unavailable)
    }
}

fn suggestion(value: &str, max: usize) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control))
        .then_some(value)
}

fn suggested_url(base: &Url, value: &str) -> Option<String> {
    let joined = base.join(value.trim()).ok()?;
    public_url(joined.as_str()).ok().map(Into::into)
}

fn extract(url: &Url, html: &str) -> MetadataPreview {
    let document = Html::parse_document(html);
    // Fixed selectors; parser failures simply preserve manual entry.
    let meta_selector = Selector::parse("meta").ok();
    let mut meta = std::collections::HashMap::new();
    if let Some(selector) = &meta_selector {
        for element in document.select(selector) {
            if let (Some(name), Some(value)) = (
                element
                    .value()
                    .attr("property")
                    .or_else(|| element.value().attr("name")),
                element.value().attr("content"),
            ) {
                meta.entry(name.to_ascii_lowercase()).or_insert(value);
            }
        }
    }
    let mut title = meta
        .get("og:title")
        .or_else(|| meta.get("twitter:title"))
        .and_then(|value| suggestion(value, 500));
    if title.is_none()
        && let Ok(selector) = Selector::parse("title")
    {
        title = document
            .select(&selector)
            .next()
            .and_then(|element| suggestion(&element.text().collect::<String>(), 500));
    }
    let image_url = meta
        .get("og:image")
        .or_else(|| meta.get("twitter:image"))
        .and_then(|value| suggested_url(url, value));
    let price = meta
        .get("product:price:amount")
        .or_else(|| meta.get("og:price:amount"))
        .and_then(|amount| {
            let currency = meta
                .get("product:price:currency")
                .or_else(|| meta.get("og:price:currency"))?;
            suggestion(&format!("{currency} {amount}"), 100)
        });
    // Keep the fetched URL, not untrusted canonical/og:url destinations.
    MetadataPreview {
        status: if title.is_some() && image_url.is_some() && price.is_some() {
            MetadataStatus::Ready
        } else {
            MetadataStatus::Partial
        },
        url: url.as_str().into(),
        title,
        image_url,
        price,
    }
}

#[cfg(test)]
mod tests;
