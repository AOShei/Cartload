//! TheGamesDB v1 client: request building, response parsing and the allowance limiter.
//!
//! The API (`https://api.thegamesdb.net/v1`, GPL-3 source at TheGamesDB/TheGamesDBv2) counts
//! every call against a monthly allowance per key (and per client address for the public keys
//! applications ship). Each answer carries `remaining_monthly_allowance`, `extra_allowance`
//! and `allowance_refresh_timer` (seconds until the count resets, `null` for private keys) in
//! the JSON body, and an exhausted key gets HTTP 403 with `remaining_monthly_allowance: 0`.
//! Image downloads from the CDN are free.

use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

use crate::{Error, Result, rom::System};

/// Base URL of the v1 API.
pub const API_BASE: &str = "https://api.thegamesdb.net/v1";
/// Base URL of the image CDN; the API answers with the per-size variants.
pub const CDN_BASE: &str = "https://cdn.thegamesdb.net/images/";
/// Environment variable holding the API key.
pub const KEY_ENV: &str = "CARTLOAD_TGDB_KEY";
/// Ids per request: the API pages results by 20.
pub const BATCH: usize = 20;
/// Game fields requested on top of the id, title and release date the API always sends.
pub const FIELDS: &str = "players,publishers,overview,platform,coop,alternates";

/// TheGamesDB platform id of the Super Nintendo (SNES).
pub const PLATFORM_SNES: u32 = 6;
/// TheGamesDB platform id of the Nintendo Entertainment System (NES).
pub const PLATFORM_NES: u32 = 7;
/// TheGamesDB platform id of the Sega Genesis.
pub const PLATFORM_GENESIS: u32 = 18;
/// TheGamesDB platform id of the Sega Mega Drive.
pub const PLATFORM_MEGA_DRIVE: u32 = 36;
/// TheGamesDB platform id of the N64.
pub const PLATFORM_N64: u32 = 3;
/// TheGamesDB platform id of the Game Boy.
pub const PLATFORM_GAME_BOY: u32 = 4;
/// TheGamesDB platform id of the Game Boy Advance.
pub const PLATFORM_GAME_BOY_ADVANCE: u32 = 5;
/// TheGamesDB platform id of the Game Boy Color.
pub const PLATFORM_GAME_BOY_COLOR: u32 = 41;

/// The platforms a system's games are listed under.
pub fn platform_ids(system: System) -> &'static [u32] {
    match system {
        System::Snes => &[PLATFORM_SNES],
        System::Nes => &[PLATFORM_NES],
        System::MegaDrive => &[PLATFORM_GENESIS, PLATFORM_MEGA_DRIVE],
        System::GameBoy => &[PLATFORM_GAME_BOY],
        System::GameBoyColor => &[PLATFORM_GAME_BOY_COLOR, PLATFORM_GAME_BOY],
        System::GameBoyAdvance => &[PLATFORM_GAME_BOY_ADVANCE],
        System::N64 => &[PLATFORM_N64],
    }
}

/// An API key. Never printed: `Debug` and `Display` hide it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// A key from text; `None` when blank.
    pub fn new(key: &str) -> Option<Self> {
        let key = key.trim();
        (!key.is_empty()).then(|| Self(key.to_string()))
    }

    /// The key from the [`KEY_ENV`] environment variable.
    pub fn from_env() -> Option<Self> {
        std::env::var(KEY_ENV).ok().and_then(|k| Self::new(&k))
    }

    /// The key to use: `explicit`, else the environment, else the stored one.
    pub fn resolve(explicit: Option<&str>, stored: Option<&str>) -> Option<Self> {
        explicit
            .and_then(Self::new)
            .or_else(Self::from_env)
            .or_else(|| stored.and_then(Self::new))
    }

    /// The key itself, for requests.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey(***)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

/// An HTTP answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// Body.
    pub body: Vec<u8>,
}

/// How requests reach the network. Implemented by [`HttpTransport`] (reqwest) and by scripted
/// transports in tests.
pub trait Transport: Send + Sync {
    /// Performs a GET and returns the status and body, following redirects.
    fn get(&self, url: &str) -> impl Future<Output = Result<Response>> + Send;
}

/// The reqwest transport.
#[cfg(feature = "fetch")]
#[derive(Clone, Debug)]
pub struct HttpTransport {
    client: reqwest::Client,
}

#[cfg(feature = "fetch")]
impl HttpTransport {
    /// A client with the crate's user agent and timeouts (rustls on ring, as everywhere in
    /// Cartload).
    pub fn new() -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/cartload/cartload)"
            ))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| Error::Transport {
                url: API_BASE.to_string(),
                message: e.to_string(),
            })?;
        Ok(Self { client })
    }
}

#[cfg(feature = "fetch")]
impl Transport for HttpTransport {
    async fn get(&self, url: &str) -> Result<Response> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Transport {
                url: redact(url),
                message: chain(&e),
            })?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|e| Error::Transport {
                url: redact(url),
                message: chain(&e),
            })?
            .to_vec();
        Ok(Response { status, body })
    }
}

#[cfg(feature = "fetch")]
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(next) = source {
        let message = next.to_string();
        if !text.contains(&message) {
            text.push_str(": ");
            text.push_str(&message);
        }
        source = next.source();
    }
    text
}

/// The URL with the key replaced by `***`.
pub fn redact(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let mut rest = url;
    while let Some(pos) = rest.find("apikey=") {
        let end = pos + "apikey=".len();
        out.push_str(&rest[..end]);
        out.push_str("***");
        let after = &rest[end..];
        rest = match after.find('&') {
            Some(amp) => &after[amp..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// `3 days`, `2 hours`, `15 minutes`, `40 seconds`.
pub fn human_duration(secs: u64) -> String {
    let (value, unit) = if secs >= 86_400 {
        (secs / 86_400, "day")
    } else if secs >= 3_600 {
        (secs / 3_600, "hour")
    } else if secs >= 60 {
        (secs / 60, "minute")
    } else {
        (secs, "second")
    };
    format!("{value} {unit}{}", if value == 1 { "" } else { "s" })
}

fn unix_now() -> i64 {
    crate::library::unix_seconds(SystemTime::now())
}

/// What the API last said about the key's monthly allowance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowance {
    /// Requests left this month; `None` until an answer has been seen.
    #[serde(default)]
    pub remaining: Option<u32>,
    /// Extra requests granted on top of the monthly ones.
    #[serde(default)]
    pub extra: u32,
    /// When the monthly count resets, seconds since the Unix epoch; `None` for private keys.
    #[serde(default)]
    pub refresh_at: Option<i64>,
    /// When this was last updated, seconds since the Unix epoch.
    #[serde(default)]
    pub updated_at: i64,
}

impl Allowance {
    /// Whether nothing is left and the reset has not happened yet (`now` in Unix seconds).
    /// Without a reset time, an exhausted allowance is trusted for a day.
    pub fn exhausted_at(&self, now: i64) -> bool {
        self.remaining == Some(0)
            && self.extra == 0
            && match self.refresh_at {
                Some(at) => now < at,
                None => now.saturating_sub(self.updated_at) < 86_400,
            }
    }

    /// Seconds until the reset, when known.
    pub fn refresh_in(&self, now: i64) -> Option<u64> {
        self.refresh_at
            .map(|at| u64::try_from(at.saturating_sub(now)).unwrap_or(0))
    }

    /// Takes the allowance fields of an API answer (`now` in Unix seconds).
    pub fn absorb(&mut self, body: &Value, now: i64) {
        let Some(remaining) = body.get("remaining_monthly_allowance").and_then(as_u32) else {
            return;
        };
        self.remaining = Some(remaining);
        if let Some(extra) = body.get("extra_allowance").and_then(as_u32) {
            self.extra = extra;
        } else if remaining == 0 {
            // The exhausted answer omits the field; the key would be using it otherwise.
            self.extra = 0;
        }
        self.refresh_at = match body.get("allowance_refresh_timer") {
            Some(timer) if !timer.is_null() => as_i64(timer).map(|secs| now + secs),
            Some(_) => None,
            None => self.refresh_at,
        };
        self.updated_at = now;
    }

    /// One line for the user: `2951 requests left this month (resets in 12 days)`.
    pub fn describe(&self, now: i64) -> String {
        match self.remaining {
            None => "TheGamesDB allowance not known yet".to_string(),
            Some(remaining) => {
                let mut text = format!("TheGamesDB: {remaining} requests left this month");
                if self.extra > 0 {
                    text.push_str(&format!(" plus {} extra", self.extra));
                }
                if let Some(secs) = self.refresh_in(now) {
                    text.push_str(&format!(" (resets in {})", human_duration(secs)));
                }
                text
            }
        }
    }
}

impl fmt::Display for Allowance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe(unix_now()))
    }
}

struct LimiterState {
    allowance: Allowance,
    last_request: Option<Instant>,
}

/// Refuses requests once the allowance is used up and keeps a minimum interval between
/// requests. State is shared by all requests of a [`Client`].
pub struct Limiter {
    state: Mutex<LimiterState>,
    min_interval: Duration,
}

impl Limiter {
    /// A limiter with the given minimum interval between requests.
    pub fn new(min_interval: Duration) -> Self {
        Self {
            state: Mutex::new(LimiterState {
                allowance: Allowance::default(),
                last_request: None,
            }),
            min_interval,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, LimiterState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The allowance as last seen.
    pub fn allowance(&self) -> Allowance {
        self.lock().allowance.clone()
    }

    /// Restores an allowance saved earlier.
    pub fn set_allowance(&self, allowance: Allowance) {
        self.lock().allowance = allowance;
    }

    /// Checks the allowance and returns how long to wait before sending.
    pub fn admit(&self, now: i64) -> Result<Duration> {
        let mut state = self.lock();
        if state.allowance.exhausted_at(now) {
            return Err(Error::AllowanceExhausted {
                refresh_in_secs: state.allowance.refresh_in(now),
            });
        }
        let wait = match state.last_request {
            Some(last) => self.min_interval.saturating_sub(last.elapsed()),
            None => Duration::ZERO,
        };
        state.last_request = Some(Instant::now() + wait);
        Ok(wait)
    }

    /// Records what an answer said about the allowance.
    pub fn record(&self, body: &Value, now: i64) {
        self.lock().allowance.absorb(body, now);
    }
}

/// Image size variants the CDN serves (`cdn.thegamesdb.net/images/<size>/...`). For game 1255
/// the front box art is 2100×1533 original, 1479×1080 large, 986×720 medium, 410×300 thumb
/// and 205×150 small, so `medium` is plenty for the console's 228×204 cover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtSize {
    /// As uploaded.
    Original,
    /// About 200 pixels on the long edge.
    Small,
    /// About 400 pixels on the long edge.
    Thumb,
    /// A square crop of the thumbnail.
    CroppedCenterThumb,
    /// About 1000 pixels on the long edge.
    #[default]
    Medium,
    /// About 1500 pixels on the long edge.
    Large,
}

impl ArtSize {
    /// The key the API uses for this size.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Small => "small",
            Self::Thumb => "thumb",
            Self::CroppedCenterThumb => "cropped_center_thumb",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

impl fmt::Display for ArtSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ArtSize {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "original" => Ok(Self::Original),
            "small" => Ok(Self::Small),
            "thumb" => Ok(Self::Thumb),
            "cropped_center_thumb" => Ok(Self::CroppedCenterThumb),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            other => Err(Error::Invalid(format!(
                "{other:?} is not an image size (original, small, thumb, cropped_center_thumb, medium, large)"
            ))),
        }
    }
}

/// The per-size base URLs of an answer (`include.boxart.base_url`, `data.base_url`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUrls {
    /// `https://cdn.thegamesdb.net/images/original/`.
    pub original: String,
    /// The `small` variant.
    pub small: String,
    /// The `thumb` variant.
    pub thumb: String,
    /// The `cropped_center_thumb` variant.
    pub cropped_center_thumb: String,
    /// The `medium` variant.
    pub medium: String,
    /// The `large` variant.
    pub large: String,
}

impl Default for ImageUrls {
    fn default() -> Self {
        Self {
            original: format!("{CDN_BASE}original/"),
            small: format!("{CDN_BASE}small/"),
            thumb: format!("{CDN_BASE}thumb/"),
            cropped_center_thumb: format!("{CDN_BASE}cropped_center_thumb/"),
            medium: format!("{CDN_BASE}medium/"),
            large: format!("{CDN_BASE}large/"),
        }
    }
}

impl ImageUrls {
    /// The base URL of a size.
    pub fn base(&self, size: ArtSize) -> &str {
        match size {
            ArtSize::Original => &self.original,
            ArtSize::Small => &self.small,
            ArtSize::Thumb => &self.thumb,
            ArtSize::CroppedCenterThumb => &self.cropped_center_thumb,
            ArtSize::Medium => &self.medium,
            ArtSize::Large => &self.large,
        }
    }

    fn parse(value: &Value) -> Self {
        let defaults = Self::default();
        let get = |key: &str, default: String| {
            value
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or(default)
        };
        Self {
            original: get("original", defaults.original),
            small: get("small", defaults.small),
            thumb: get("thumb", defaults.thumb),
            cropped_center_thumb: get("cropped_center_thumb", defaults.cropped_center_thumb),
            medium: get("medium", defaults.medium),
            large: get("large", defaults.large),
        }
    }
}

/// One image of a game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtImage {
    /// Image id.
    pub id: u32,
    /// `boxart`, `clearlogo`, `fanart`, `banner`, `screenshot`, `titlescreen`.
    pub kind: String,
    /// `front`, `back` or `spine` for box art.
    #[serde(default)]
    pub side: Option<String>,
    /// Path under the size base URL (`boxart/front/1255-1.jpg`).
    pub filename: String,
    /// `1536x2100` when known.
    #[serde(default)]
    pub resolution: Option<String>,
}

impl ArtImage {
    /// Whether this is the front of the box.
    pub fn is_front_boxart(&self) -> bool {
        self.kind == "boxart" && self.side.as_deref() == Some("front")
    }

    /// Whether this is the clear logo.
    pub fn is_clear_logo(&self) -> bool {
        self.kind == "clearlogo"
    }

    /// The download URL in a size.
    pub fn url(&self, urls: &ImageUrls, size: ArtSize) -> String {
        format!("{}{}", urls.base(size), self.filename)
    }

    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: value.get("id").and_then(as_u32).unwrap_or(0),
            kind: value.get("type").and_then(Value::as_str)?.to_string(),
            side: value
                .get("side")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            filename: value.get("filename").and_then(Value::as_str)?.to_string(),
            resolution: value
                .get("resolution")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        })
    }
}

/// The URL hakchi2 CE downloads clear logos from without an API call: the CDN stores them as
/// `clearlogo/<game id>.png`.
pub fn clear_logo_url(id: u32) -> String {
    format!("{CDN_BASE}original/clearlogo/{id}.png")
}

/// A game as the API describes it, with the box art list of the same answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRecord {
    /// TheGamesDB game id.
    pub id: u32,
    /// `game_title`.
    pub title: String,
    /// Platform id.
    #[serde(default)]
    pub platform: Option<u32>,
    /// `YYYY-MM-DD` when known.
    #[serde(default)]
    pub release_date: Option<String>,
    /// Number of players when known and above zero.
    #[serde(default)]
    pub players: Option<u32>,
    /// Whether the API lists co-operative play.
    #[serde(default)]
    pub coop: Option<bool>,
    /// The description.
    #[serde(default)]
    pub overview: Option<String>,
    /// Publisher ids (names come from the publishers list).
    #[serde(default)]
    pub publisher_ids: Vec<u32>,
    /// Developer ids.
    #[serde(default)]
    pub developer_ids: Vec<u32>,
    /// Alternative titles.
    #[serde(default)]
    pub alternates: Vec<String>,
    /// Base URLs for [`Self::images`].
    #[serde(default)]
    pub image_urls: ImageUrls,
    /// The images the answer listed (box art with `include=boxart`).
    #[serde(default)]
    pub images: Vec<ArtImage>,
}

impl GameRecord {
    /// The front box art, when listed.
    pub fn front_boxart(&self) -> Option<&ArtImage> {
        self.images.iter().find(|i| i.is_front_boxart())
    }

    /// The clear logo, when listed.
    pub fn clear_logo(&self) -> Option<&ArtImage> {
        self.images.iter().find(|i| i.is_clear_logo())
    }

    /// The release year, when the date is known.
    pub fn release_year(&self) -> Option<&str> {
        self.release_date
            .as_deref()
            .filter(|d| d.len() >= 4 && d[..4].bytes().all(|b| b.is_ascii_digit()))
            .map(|d| &d[..4])
    }

    fn parse(value: &Value, urls: &ImageUrls, boxart: Option<&Value>) -> Option<Self> {
        let id = value.get("id").and_then(as_u32)?;
        let title = value
            .get("game_title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let images = boxart
            .and_then(|data| data.get(id.to_string()))
            .map(|list| values_of(list).filter_map(ArtImage::parse).collect())
            .unwrap_or_default();
        Some(Self {
            id,
            title,
            platform: value.get("platform").and_then(as_u32),
            release_date: value
                .get("release_date")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string),
            players: value.get("players").and_then(as_u32).filter(|p| *p > 0),
            coop: value
                .get("coop")
                .and_then(Value::as_str)
                .map(|c| c.eq_ignore_ascii_case("yes")),
            overview: value
                .get("overview")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(str::to_string),
            publisher_ids: value.get("publishers").map(as_u32_list).unwrap_or_default(),
            developer_ids: value.get("developers").map(as_u32_list).unwrap_or_default(),
            alternates: value
                .get("alternates")
                .map(|v| {
                    values_of(v)
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            image_urls: urls.clone(),
            images,
        })
    }
}

/// The images of several games, from `Games/Images`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageSet {
    /// Base URLs.
    pub urls: ImageUrls,
    /// Images per game id.
    pub by_game: BTreeMap<u32, Vec<ArtImage>>,
}

/// A page of games.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GamesPage {
    /// The games with their box art.
    pub games: Vec<GameRecord>,
    /// URL of the next page, when there is one.
    pub next: Option<String>,
}

/// Number or numeric string, as PHP may send either.
pub(crate) fn as_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The values of an array or of an object (PHP encodes empty maps as `[]`).
fn values_of(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match value {
        Value::Array(items) => Box::new(items.iter()),
        Value::Object(map) => Box::new(map.values()),
        _ => Box::new(std::iter::empty()),
    }
}

fn as_u32_list(value: &Value) -> Vec<u32> {
    values_of(value).filter_map(as_u32).collect()
}

/// Parses `pages.next`.
fn next_page(value: &Value) -> Option<String> {
    value
        .pointer("/pages/next")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Parses a `Games/*` answer.
pub fn parse_games_page(value: &Value) -> GamesPage {
    let urls = value
        .pointer("/include/boxart/base_url")
        .map(ImageUrls::parse)
        .unwrap_or_default();
    let boxart = value.pointer("/include/boxart/data");
    let games = value
        .pointer("/data/games")
        .map(|list| {
            values_of(list)
                .filter_map(|g| GameRecord::parse(g, &urls, boxart))
                .collect()
        })
        .unwrap_or_default();
    GamesPage {
        games,
        next: next_page(value),
    }
}

/// Parses a `Games/Images` answer into `set`; returns the next page URL.
pub fn parse_images_page(value: &Value, set: &mut ImageSet) -> Option<String> {
    if let Some(urls) = value.pointer("/data/base_url") {
        set.urls = ImageUrls::parse(urls);
    }
    if let Some(Value::Object(images)) = value.pointer("/data/images") {
        for (id, list) in images {
            if let Ok(id) = id.trim().parse::<u32>() {
                set.by_game
                    .entry(id)
                    .or_default()
                    .extend(values_of(list).filter_map(ArtImage::parse));
            }
        }
    }
    next_page(value)
}

/// Parses a `Publishers`, `Developers` or `Genres` list into id → name.
pub fn parse_names(value: &Value, key: &str) -> BTreeMap<u32, String> {
    value
        .pointer(&format!("/data/{key}"))
        .map(|list| {
            values_of(list)
                .filter_map(|entry| {
                    let id = entry.get("id").and_then(as_u32)?;
                    let name = entry.get("name").and_then(Value::as_str)?.trim();
                    (!name.is_empty()).then(|| (id, name.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Percent-encodes a query value (RFC 3986 unreserved characters kept).
pub fn encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn join_ids(ids: &[u32]) -> String {
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// The client: one key, one transport, one limiter.
pub struct Client<T: Transport> {
    transport: T,
    key: ApiKey,
    base: String,
    limiter: Limiter,
}

impl<T: Transport> Client<T> {
    /// A client for the public API with a quarter-second gap between requests.
    pub fn new(transport: T, key: ApiKey) -> Self {
        Self {
            transport,
            key,
            base: API_BASE.to_string(),
            limiter: Limiter::new(Duration::from_millis(250)),
        }
    }

    /// Uses another base URL (a mirror, a test server).
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.trim_end_matches('/').to_string();
        self
    }

    /// Uses another minimum interval between requests.
    pub fn with_min_interval(mut self, interval: Duration) -> Self {
        self.limiter = Limiter::new(interval);
        self
    }

    /// The limiter, to restore or read the allowance.
    pub fn limiter(&self) -> &Limiter {
        &self.limiter
    }

    /// The allowance as last seen.
    pub fn allowance(&self) -> Allowance {
        self.limiter.allowance()
    }

    /// The transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// The URL of a route with the key and the parameters.
    pub fn url(&self, route: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/{}?apikey={}",
            self.base,
            route.trim_start_matches('/'),
            encode_query(self.key.as_str())
        );
        for (name, value) in params {
            url.push('&');
            url.push_str(&encode_query(name).replace("%5B", "[").replace("%5D", "]"));
            url.push('=');
            url.push_str(&encode_query(value));
        }
        url
    }

    /// One API call: waits for the limiter, checks the status and records the allowance.
    pub async fn get_json(&self, url: &str) -> Result<Value> {
        let wait = self.limiter.admit(unix_now())?;
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        debug!(url = %redact(url), "TheGamesDB request");
        let response = self.transport.get(url).await?;
        let endpoint = endpoint_of(url);
        let body: Value = match serde_json::from_slice(&response.body) {
            Ok(body) => body,
            Err(error) => {
                return Err(Error::Api {
                    endpoint,
                    status: response.status,
                    message: format!("not JSON ({error})"),
                });
            }
        };
        let now = unix_now();
        let code = body
            .get("code")
            .and_then(as_u32)
            .map_or(response.status, |c| c as u16);
        let message = body
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // The middleware answers 401 (timer 0) for an unknown key and 403 with the time to
        // the reset (`null` for private keys, which never reset) for a used-up one. The
        // allowance fields of a rejected key mean nothing and are not recorded.
        let timer = body.get("allowance_refresh_timer");
        let exhausted = code == 403
            && body
                .get("remaining_monthly_allowance")
                .and_then(as_u32)
                .is_some_and(|r| r == 0)
            && timer.is_some_and(|t| t.is_null() || as_i64(t).is_some_and(|secs| secs > 0));
        match code {
            200..=299 => {
                self.limiter.record(&body, now);
                Ok(body)
            }
            403 if exhausted => {
                self.limiter.record(&body, now);
                Err(Error::AllowanceExhausted {
                    refresh_in_secs: self.limiter.allowance().refresh_in(now),
                })
            }
            401 | 403 => Err(Error::InvalidApiKey {
                status: code,
                message,
            }),
            _ => Err(Error::Api {
                endpoint,
                status: code,
                message,
            }),
        }
    }

    /// Searches games by name on the given platforms (first page, 20 results at most), with
    /// box art.
    pub async fn search(&self, name: &str, platforms: &[u32]) -> Result<GamesPage> {
        let platforms = join_ids(platforms);
        let mut params = vec![
            ("name", name),
            ("fields", FIELDS),
            ("include", "boxart,platform"),
        ];
        if !platforms.is_empty() {
            params.push(("filter[platform]", platforms.as_str()));
        }
        let url = self.url("Games/ByGameName", &params);
        Ok(parse_games_page(&self.get_json(&url).await?))
    }

    /// Fetches games by id, with box art, in batches of [`BATCH`] and across pages.
    pub async fn games(&self, ids: &[u32]) -> Result<Vec<GameRecord>> {
        let mut games = Vec::new();
        for chunk in ids.chunks(BATCH) {
            let ids = join_ids(chunk);
            let mut url = self.url(
                "Games/ByGameID",
                &[
                    ("id", ids.as_str()),
                    ("fields", FIELDS),
                    ("include", "boxart,platform"),
                ],
            );
            loop {
                let page = parse_games_page(&self.get_json(&url).await?);
                games.extend(page.games);
                match page.next {
                    Some(next) => url = next,
                    None => break,
                }
            }
        }
        Ok(games)
    }

    /// Fetches the images of games (`boxart`, `clearlogo`, `fanart`, `banner`, `screenshot`,
    /// `titlescreen`; all when `kinds` is empty).
    pub async fn images(&self, ids: &[u32], kinds: &[&str]) -> Result<ImageSet> {
        let mut set = ImageSet::default();
        let kinds = kinds.join(",");
        for chunk in ids.chunks(BATCH) {
            let ids = join_ids(chunk);
            let mut params = vec![("games_id", ids.as_str())];
            if !kinds.is_empty() {
                params.push(("filter[type]", kinds.as_str()));
            }
            let mut url = self.url("Games/Images", &params);
            while let Some(next) = parse_images_page(&self.get_json(&url).await?, &mut set) {
                url = next;
            }
        }
        Ok(set)
    }

    /// The publishers list, id → name.
    pub async fn publishers(&self) -> Result<BTreeMap<u32, String>> {
        let url = self.url("Publishers", &[]);
        Ok(parse_names(&self.get_json(&url).await?, "publishers"))
    }

    /// The developers list, id → name.
    pub async fn developers(&self) -> Result<BTreeMap<u32, String>> {
        let url = self.url("Developers", &[]);
        Ok(parse_names(&self.get_json(&url).await?, "developers"))
    }

    /// Downloads a file (an image from the CDN; no allowance involved). `None` when the
    /// server has no such file.
    pub async fn download(&self, url: &str) -> Result<Option<Vec<u8>>> {
        debug!(url, "download");
        let response = self.transport.get(url).await?;
        match response.status {
            200..=299 => Ok(Some(response.body)),
            404 => Ok(None),
            status => {
                warn!(url, status, "download failed");
                Err(Error::Api {
                    endpoint: url.to_string(),
                    status,
                    message: "download failed".to_string(),
                })
            }
        }
    }
}

/// `/Games/ByGameID` from a full URL, for messages.
fn endpoint_of(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    match without_query.find("/v1") {
        Some(pos) => without_query[pos..].to_string(),
        None => without_query.to_string(),
    }
}

/// Scripted transports for tests: URL → answer, with a log of the requests made.
#[cfg(test)]
pub(crate) mod testing {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    /// Answers from a table; unknown URLs get 404 with an empty body.
    #[derive(Default)]
    pub(crate) struct Scripted {
        answers: HashMap<String, (u16, Vec<u8>)>,
        requests: Mutex<Vec<String>>,
    }

    impl Scripted {
        pub(crate) fn answer(mut self, url: &str, status: u16, body: impl Into<Vec<u8>>) -> Self {
            self.answers.insert(url.to_string(), (status, body.into()));
            self
        }

        pub(crate) fn requests(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
        }
    }

    impl Transport for Scripted {
        async fn get(&self, url: &str) -> Result<Response> {
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(url.to_string());
            let (status, body) = self.answers.get(url).cloned().unwrap_or((404, Vec::new()));
            Ok(Response { status, body })
        }
    }

    pub(crate) const KEY: &str = "test-key";

    pub(crate) fn client(scripted: Scripted) -> Client<Scripted> {
        Client::new(scripted, ApiKey::new(KEY).expect("key")).with_min_interval(Duration::ZERO)
    }

    pub(crate) fn fixture(name: &str) -> &'static str {
        match name {
            "by-game-name-chrono-trigger" => {
                include_str!("../../tests/fixtures/tgdb/by-game-name-chrono-trigger.json")
            }
            "by-game-name-empty" => {
                include_str!("../../tests/fixtures/tgdb/by-game-name-empty.json")
            }
            "by-game-id-1255" => include_str!("../../tests/fixtures/tgdb/by-game-id-1255.json"),
            "by-game-id-page-1" => include_str!("../../tests/fixtures/tgdb/by-game-id-page-1.json"),
            "by-game-id-page-2" => include_str!("../../tests/fixtures/tgdb/by-game-id-page-2.json"),
            "images-1255" => include_str!("../../tests/fixtures/tgdb/images-1255.json"),
            "publishers" => include_str!("../../tests/fixtures/tgdb/publishers.json"),
            "developers" => include_str!("../../tests/fixtures/tgdb/developers.json"),
            "error-401" => include_str!("../../tests/fixtures/tgdb/error-401.json"),
            "error-403-invalid-key" => {
                include_str!("../../tests/fixtures/tgdb/error-403-invalid-key.json")
            }
            "error-403-exhausted" => {
                include_str!("../../tests/fixtures/tgdb/error-403-exhausted.json")
            }
            other => panic!("no fixture {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{testing::*, *};

    #[test]
    fn keys_are_resolved_in_order_and_never_shown() {
        assert!(ApiKey::new("  ").is_none());
        let key = ApiKey::new(" abc ").unwrap();
        assert_eq!(key.as_str(), "abc");
        assert_eq!(format!("{key:?}"), "ApiKey(***)");
        assert_eq!(key.to_string(), "***");
        // The environment is not touched in tests; explicit wins, then stored.
        assert_eq!(
            ApiKey::resolve(Some("x"), Some("y")).map(|k| k.0),
            Some("x".into())
        );
        if std::env::var_os(KEY_ENV).is_none() {
            assert_eq!(
                ApiKey::resolve(None, Some("y")).map(|k| k.0),
                Some("y".into())
            );
            assert!(ApiKey::resolve(None, None).is_none());
        }
    }

    #[test]
    fn urls_carry_the_key_and_encode_parameters() {
        let client = client(Scripted::default());
        assert_eq!(
            client.url(
                "Games/ByGameName",
                &[
                    ("name", "Kirby's Dream Land 3 & more"),
                    ("filter[platform]", "6,7")
                ]
            ),
            "https://api.thegamesdb.net/v1/Games/ByGameName?apikey=test-key&name=Kirby%27s%20Dream%20Land%203%20%26%20more&filter[platform]=6%2C7"
        );
        assert_eq!(
            redact("https://api.thegamesdb.net/v1/Games/ByGameID?apikey=secret&id=1&apikey=secret"),
            "https://api.thegamesdb.net/v1/Games/ByGameID?apikey=***&id=1&apikey=***"
        );
        assert_eq!(
            redact("https://cdn.example/x.png"),
            "https://cdn.example/x.png"
        );
        assert_eq!(
            endpoint_of("https://api.thegamesdb.net/v1/Games/ByGameID?apikey=k&id=1"),
            "/v1/Games/ByGameID"
        );
        assert_eq!(human_duration(0), "0 seconds");
        assert_eq!(human_duration(1), "1 second");
        assert_eq!(human_duration(59), "59 seconds");
        assert_eq!(human_duration(3_600), "1 hour");
        assert_eq!(human_duration(2_591_000), "29 days");
        assert_eq!(platform_ids(System::MegaDrive), &[18, 36]);
    }

    #[test]
    fn allowance_follows_the_body_fields() {
        let mut allowance = Allowance::default();
        assert!(!allowance.exhausted_at(1_000));
        allowance.absorb(
            &serde_json::json!({"remaining_monthly_allowance": 5, "extra_allowance": 0, "allowance_refresh_timer": 100}),
            1_000,
        );
        assert_eq!(allowance.remaining, Some(5));
        assert_eq!(allowance.refresh_at, Some(1_100));
        assert!(!allowance.exhausted_at(1_000));
        assert_eq!(
            allowance.describe(1_050),
            "TheGamesDB: 5 requests left this month (resets in 50 seconds)"
        );
        // The exhausted answer: remaining 0, no extra field, a timer.
        allowance.absorb(
            &serde_json::json!({"code": 403, "remaining_monthly_allowance": 0, "allowance_refresh_timer": 60}),
            2_000,
        );
        assert!(allowance.exhausted_at(2_030));
        assert!(!allowance.exhausted_at(2_061), "the reset time has passed");
        assert_eq!(allowance.refresh_in(2_030), Some(30));
        // Extra allowance keeps requests going.
        allowance.absorb(
            &serde_json::json!({"remaining_monthly_allowance": 0, "extra_allowance": 3, "allowance_refresh_timer": 60}),
            3_000,
        );
        assert!(!allowance.exhausted_at(3_000));
        assert_eq!(
            allowance.describe(3_000),
            "TheGamesDB: 0 requests left this month plus 3 extra (resets in 1 minute)"
        );
        // Private keys have no timer: an exhausted answer is trusted for a day.
        allowance.absorb(
            &serde_json::json!({"remaining_monthly_allowance": 0, "extra_allowance": 0, "allowance_refresh_timer": null}),
            4_000,
        );
        assert_eq!(allowance.refresh_at, None);
        assert!(allowance.exhausted_at(4_000 + 3_600));
        assert!(!allowance.exhausted_at(4_000 + 90_000));
        // Numbers as strings, as PHP may send them.
        allowance.absorb(
            &serde_json::json!({"remaining_monthly_allowance": "12", "extra_allowance": "1"}),
            5_000,
        );
        assert_eq!((allowance.remaining, allowance.extra), (Some(12), 1));
        assert_eq!(
            Allowance::default().describe(0),
            "TheGamesDB allowance not known yet"
        );
    }

    #[test]
    fn limiter_refuses_when_exhausted_and_spaces_requests() {
        let limiter = Limiter::new(Duration::from_millis(100));
        assert_eq!(limiter.admit(0).unwrap(), Duration::ZERO);
        let wait = limiter.admit(0).unwrap();
        assert!(wait > Duration::ZERO && wait <= Duration::from_millis(100));
        limiter.record(
            &serde_json::json!({"remaining_monthly_allowance": 0, "extra_allowance": 0, "allowance_refresh_timer": 500}),
            1_000,
        );
        assert!(matches!(
            limiter.admit(1_001),
            Err(Error::AllowanceExhausted {
                refresh_in_secs: Some(499)
            })
        ));
        assert!(limiter.admit(1_600).is_ok());
        limiter.set_allowance(Allowance {
            remaining: Some(2),
            ..Allowance::default()
        });
        assert_eq!(limiter.allowance().remaining, Some(2));
    }

    #[tokio::test]
    async fn search_parses_games_and_boxart() {
        let scripted = Scripted::default().answer(
            "https://api.thegamesdb.net/v1/Games/ByGameName?apikey=test-key&name=Chrono%20Trigger&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform&filter[platform]=6",
            200,
            fixture("by-game-name-chrono-trigger"),
        );
        let client = client(scripted);
        let page = client
            .search("Chrono Trigger", &[PLATFORM_SNES])
            .await
            .unwrap();
        assert_eq!(page.next, None);
        assert_eq!(page.games.len(), 3);
        let hack = &page.games[0];
        assert_eq!(hack.id, 90210);
        assert_eq!(hack.title, "Chrono Trigger: Crimson Echoes");
        assert!(hack.images.is_empty());
        let game = &page.games[1];
        assert_eq!(game.id, 1255);
        assert_eq!(game.title, "Chrono Trigger");
        assert_eq!(game.platform, Some(6));
        assert_eq!(game.release_date.as_deref(), Some("1995-03-11"));
        assert_eq!(game.players, Some(1));
        assert_eq!(game.coop, Some(false));
        assert_eq!(game.publisher_ids, [4365]);
        assert_eq!(game.developer_ids, [1428]);
        assert_eq!(game.alternates, ["Chrono Trigger (JP)"]);
        assert_eq!(game.release_year(), Some("1995"));
        assert!(game.overview.as_deref().unwrap().starts_with("Crono"));
        assert_eq!(game.images.len(), 3);
        let front = game.front_boxart().unwrap();
        assert_eq!(front.filename, "boxart/front/1255-1.jpg");
        assert_eq!(front.resolution.as_deref(), Some("2100x1533"));
        assert_eq!(
            front.url(&game.image_urls, ArtSize::Medium),
            "https://cdn.thegamesdb.net/images/medium/boxart/front/1255-1.jpg"
        );
        assert_eq!(
            front.url(&game.image_urls, ArtSize::Original),
            "https://cdn.thegamesdb.net/images/original/boxart/front/1255-1.jpg"
        );
        assert!(
            game.clear_logo().is_none(),
            "box art includes carry no logos"
        );
        assert_eq!(client.allowance().remaining, Some(2951));
        assert_eq!(client.allowance().extra, 0);
        assert_eq!(page.games[2].players, None, "players 0 is unknown");
        assert_eq!(page.games[2].release_date, None);
    }

    #[tokio::test]
    async fn empty_searches_have_no_include_block() {
        let scripted = Scripted::default().answer(
            "https://api.thegamesdb.net/v1/Games/ByGameName?apikey=test-key&name=Nothing&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform",
            200,
            fixture("by-game-name-empty"),
        );
        let client = client(scripted);
        let page = client.search("Nothing", &[]).await.unwrap();
        assert!(page.games.is_empty());
        assert_eq!(client.allowance().remaining, Some(2950));
    }

    #[tokio::test]
    async fn games_follow_pages_and_batch_ids() {
        let base = "https://api.thegamesdb.net/v1/Games/ByGameID?apikey=test-key&id=1255&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform";
        let scripted = Scripted::default()
            .answer(base, 200, fixture("by-game-id-page-1"))
            .answer(
                "https://api.thegamesdb.net/v1/Games/ByGameID?apikey=test-key&id=1255&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform&page=2",
                200,
                fixture("by-game-id-page-2"),
            );
        let client = client(scripted);
        let games = client.games(&[1255]).await.unwrap();
        assert_eq!(games.iter().map(|g| g.id).collect::<Vec<_>>(), [1255, 1256]);
        assert_eq!(client.transport().requests().len(), 2);
        // 25 ids go out as 20 + 5.
        let ids: Vec<u32> = (1..=25).collect();
        let error = client.games(&ids).await.unwrap_err();
        assert!(matches!(error, Error::Api { status: 404, .. }), "{error}");
        let requests = client.transport().requests();
        assert!(requests[2].contains("&id=1%2C2%2C3%2C4%2C5%2C6%2C7%2C8%2C9%2C10%2C11%2C12%2C13%2C14%2C15%2C16%2C17%2C18%2C19%2C20&"));
    }

    #[tokio::test]
    async fn images_lists_every_kind_per_game() {
        let scripted = Scripted::default().answer(
            "https://api.thegamesdb.net/v1/Games/Images?apikey=test-key&games_id=1255&filter[type]=boxart%2Cclearlogo",
            200,
            fixture("images-1255"),
        );
        let client = client(scripted);
        let set = client
            .images(&[1255], &["boxart", "clearlogo"])
            .await
            .unwrap();
        let images = &set.by_game[&1255];
        assert_eq!(images.len(), 3);
        let logo = images.iter().find(|i| i.is_clear_logo()).unwrap();
        assert_eq!(logo.filename, "clearlogo/1255.png");
        assert_eq!(logo.url(&set.urls, ArtSize::Original), clear_logo_url(1255));
        assert_eq!(images.iter().filter(|i| i.is_front_boxart()).count(), 1);
    }

    #[tokio::test]
    async fn name_lists_and_errors() {
        let scripted = Scripted::default()
            .answer(
                "https://api.thegamesdb.net/v1/Publishers?apikey=test-key",
                200,
                fixture("publishers"),
            )
            .answer(
                "https://api.thegamesdb.net/v1/Developers?apikey=test-key",
                403,
                fixture("error-403-exhausted"),
            )
            .answer(
                "https://api.thegamesdb.net/v1/Genres?apikey=test-key",
                401,
                fixture("error-401"),
            )
            .answer(
                "https://api.thegamesdb.net/v1/Games/Updates?apikey=test-key",
                200,
                "<html>",
            )
            .answer(
                "https://cdn.thegamesdb.net/images/original/clearlogo/1.png",
                500,
                "",
            );
        let client = client(scripted);
        let publishers = client.publishers().await.unwrap();
        assert_eq!(publishers[&4365], "Square");
        assert_eq!(publishers.len(), 4);
        let error = client.developers().await.unwrap_err();
        assert!(
            matches!(error, Error::AllowanceExhausted { refresh_in_secs: Some(s) } if (1_234_000..=1_234_567).contains(&s)),
            "{error}"
        );
        assert!(
            error.to_string().contains("resets in 14 days")
                || error.to_string().contains("refreshes in 14 days")
        );
        assert!(client.allowance().exhausted_at(unix_now()));
        // Exhausted: the limiter refuses before any request goes out.
        let error = client
            .get_json(&client.url("Genres", &[]))
            .await
            .unwrap_err();
        assert!(matches!(error, Error::AllowanceExhausted { .. }));
        assert_eq!(client.transport().requests().len(), 2);
        client.limiter().set_allowance(Allowance::default());
        let error = client
            .get_json(&client.url("Genres", &[]))
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::InvalidApiKey { status: 401, .. }),
            "{error}"
        );
        let error = client
            .get_json(&client.url("Games/Updates", &[]))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, Error::Api { endpoint, status: 200, message } if endpoint == "/v1/Games/Updates" && message.starts_with("not JSON")),
            "{error}"
        );
        assert!(
            !error.to_string().contains(KEY),
            "errors never show the key"
        );
        assert_eq!(
            client
                .download("https://cdn.thegamesdb.net/images/original/clearlogo/2.png")
                .await
                .unwrap(),
            None
        );
        assert!(
            client
                .download("https://cdn.thegamesdb.net/images/original/clearlogo/1.png")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejected_keys_are_told_apart_from_used_up_ones() {
        let scripted = Scripted::default()
            .answer(
                "https://api.thegamesdb.net/v1/Genres?apikey=test-key",
                403,
                fixture("error-403-invalid-key"),
            )
            .answer(
                "https://api.thegamesdb.net/v1/Developers?apikey=test-key",
                403,
                r#"{"code":403,"status":"Invalid API key was provided.","remaining_monthly_allowance":0,"allowance_refresh_timer":null}"#,
            );
        let client = client(scripted);
        let error = client
            .get_json(&client.url("Genres", &[]))
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::InvalidApiKey { status: 403, .. }),
            "{error}"
        );
        assert_eq!(client.allowance(), Allowance::default(), "nothing recorded");
        let error = client.developers().await.unwrap_err();
        assert!(
            matches!(
                error,
                Error::AllowanceExhausted {
                    refresh_in_secs: None
                }
            ),
            "{error}"
        );
        assert_eq!(client.allowance().remaining, Some(0));
        assert_eq!(client.allowance().refresh_at, None);
    }

    #[test]
    fn art_sizes_parse_and_print() {
        assert_eq!("medium".parse::<ArtSize>().unwrap(), ArtSize::Medium);
        assert_eq!(
            "Cropped_Center_Thumb".parse::<ArtSize>().unwrap(),
            ArtSize::CroppedCenterThumb
        );
        assert!("huge".parse::<ArtSize>().is_err());
        assert_eq!(ArtSize::Large.to_string(), "large");
        assert_eq!(
            ImageUrls::default().base(ArtSize::Thumb),
            "https://cdn.thegamesdb.net/images/thumb/"
        );
        let json = serde_json::to_string(&ArtSize::CroppedCenterThumb).unwrap();
        assert_eq!(json, "\"cropped_center_thumb\"");
    }

    #[test]
    fn php_shapes_are_tolerated() {
        // Empty maps come as `[]`, numbers as strings, games as an object.
        let value = serde_json::json!({
            "code": 200, "status": "Success",
            "data": {"count": "1", "games": {"0": {"id": "7", "game_title": " X ", "players": "2", "publishers": ["3", 4]}}},
            "include": {"boxart": {"base_url": [], "data": []}},
            "pages": {"previous": null, "current": "c", "next": null}
        });
        let page = parse_games_page(&value);
        assert_eq!(page.games.len(), 1);
        assert_eq!(page.games[0].title, "X");
        assert_eq!(page.games[0].players, Some(2));
        assert_eq!(page.games[0].publisher_ids, [3, 4]);
        assert_eq!(page.games[0].image_urls, ImageUrls::default());
        assert!(page.games[0].images.is_empty());
        assert_eq!(page.next, None);
        let mut set = ImageSet::default();
        assert_eq!(
            parse_images_page(
                &serde_json::json!({"data": {"count": 0, "images": []}}),
                &mut set
            ),
            None
        );
        assert!(set.by_game.is_empty());
        assert!(
            parse_names(
                &serde_json::json!({"data": {"publishers": []}}),
                "publishers"
            )
            .is_empty()
        );
    }
}
