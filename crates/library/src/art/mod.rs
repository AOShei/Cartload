//! Cover art and metadata from TheGamesDB.
//!
//! The flow, per game: find the TheGamesDB id (the local index, then the hash database's ids,
//! then a search by name on the game's platform), fetch the game record with its box art list
//! (batched, 20 ids per request), download the front box art and the clear logo, make the
//! console's cover and thumbnail ([`crate::images`]), and fill the metadata hakchi2 CE fills
//! (`AddGamesTask.ScrapeGames`: publisher in upper case, `© year developers` when there is no
//! publisher, release date, players with two players meaning simultaneous, description).
//!
//! Everything fetched is kept under `<library>/art/` ([`index::ArtStore`]): the hash → id
//! index, the records, the downloads, the publisher and developer names and the allowance,
//! so a second import of the same ROM, or a second run, needs no search and no download.
//!
//! The API key comes from the `CARTLOAD_TGDB_KEY` environment variable or the library
//! settings ([`crate::LibraryConfig::tgdb_key`]); nothing is built in.

pub mod client;
pub mod index;

use std::collections::BTreeMap;

use serde::Serialize;
use tracing::{debug, info, warn};

#[cfg(feature = "fetch")]
pub use client::HttpTransport;
pub use client::{
    Allowance, ApiKey, ArtImage, ArtSize, Client, GameRecord, ImageUrls, KEY_ENV, Limiter,
    Response, Transport, clear_logo_url, human_duration, platform_ids,
};
pub use index::{
    ART_DIR, ArtStore, CachedRecord, Candidate, Index, IndexEntry, LogoState, MatchSource,
};

use index::{CLEAR_LOGO_FILE, MAX_CANDIDATES, NameList};

use crate::{
    Error, Game, Library, Result,
    images::CoverOptions,
    rom::{self, System},
};

/// What to fetch and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtOptions {
    /// The box art size to download (`medium` by default).
    pub size: ArtSize,
    /// How the cover and thumbnail are made.
    pub cover: CoverOptions,
    /// Fetch and apply again even when art is there and downloads are cached.
    pub force: bool,
    /// Run searches again instead of reusing their index entries (manual choices are kept).
    pub research: bool,
    /// Search with this term instead of the game's title (one game at a time).
    pub query: Option<String>,
    /// Fill the metadata (publisher, release date, players, description) from the record.
    pub metadata: bool,
    /// Replace metadata that is not at its import default any more.
    pub overwrite: bool,
    /// Download the clear logo too (kept in the cache for later use).
    pub logo: bool,
}

impl Default for ArtOptions {
    fn default() -> Self {
        Self {
            size: ArtSize::Medium,
            cover: CoverOptions::default(),
            force: false,
            research: false,
            query: None,
            metadata: true,
            overwrite: false,
            logo: true,
        }
    }
}

/// Progress of [`fetch_art`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ArtEvent {
    /// A search is going out.
    Searching {
        /// Game code.
        code: String,
        /// The search term.
        query: String,
    },
    /// The game's TheGamesDB id is known.
    Matched {
        /// Game code.
        code: String,
        /// TheGamesDB id.
        id: u32,
        /// Title on TheGamesDB.
        title: String,
        /// Where the match came from.
        source: MatchSource,
    },
    /// No search found anything.
    NoMatch {
        /// Game code.
        code: String,
        /// The terms tried.
        queries: Vec<String>,
    },
    /// A request or download is going out.
    Fetching {
        /// Game code, when the request is for one game.
        code: Option<String>,
        /// What is being fetched.
        what: String,
    },
    /// A game is done.
    Done {
        /// Game code.
        code: String,
        /// What happened.
        outcome: Outcome,
    },
    /// A game failed; the run goes on.
    Failed {
        /// Game code.
        code: String,
        /// The error.
        error: String,
    },
}

/// What happened to the cover of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverStatus {
    /// The game already had art (or nothing was found to replace it).
    Unchanged,
    /// A new cover and thumbnail were written.
    Written,
    /// The record lists no front box art; the placeholder stays.
    NoBoxArt,
    /// No TheGamesDB game matched; the placeholder stays.
    NoMatch,
}

/// The result for one game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// TheGamesDB id, when matched.
    pub id: Option<u32>,
    /// Title on TheGamesDB.
    pub title: Option<String>,
    /// Where the match came from.
    pub source: Option<MatchSource>,
    /// The cover.
    pub cover: CoverStatus,
    /// Cover dimensions when written.
    pub cover_size: Option<(u32, u32)>,
    /// Thumbnail dimensions when written.
    pub thumbnail_size: Option<(u32, u32)>,
    /// Whether the box art came from the cache rather than the network.
    pub from_cache: bool,
    /// The clear logo.
    pub logo: LogoState,
    /// Metadata fields that were filled.
    pub metadata: Vec<&'static str>,
}

impl Outcome {
    fn new(cover: CoverStatus) -> Self {
        Self {
            id: None,
            title: None,
            source: None,
            cover,
            cover_size: None,
            thumbnail_size: None,
            from_cache: false,
            logo: LogoState::Unknown,
            metadata: Vec::new(),
        }
    }
}

/// One game of a report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GameResult {
    /// Game code.
    pub code: String,
    /// What happened, when the game was processed.
    pub outcome: Option<Outcome>,
    /// The error, when it failed.
    pub error: Option<String>,
}

/// The result of [`fetch_art`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ArtReport {
    /// One entry per game, in the order given.
    pub results: Vec<GameResult>,
    /// The allowance after the run.
    pub allowance: Allowance,
}

impl ArtReport {
    fn push_outcome(&mut self, code: &str, outcome: Outcome) {
        self.results.push(GameResult {
            code: code.to_string(),
            outcome: Some(outcome),
            error: None,
        });
    }

    fn push_error(&mut self, code: &str, error: &Error) {
        self.results.push(GameResult {
            code: code.to_string(),
            outcome: None,
            error: Some(error.to_string()),
        });
    }

    /// Games whose cover was written.
    pub fn written(&self) -> usize {
        self.count(|o| o.cover == CoverStatus::Written)
    }

    /// Games that matched nothing.
    pub fn unmatched(&self) -> usize {
        self.count(|o| o.cover == CoverStatus::NoMatch)
    }

    /// Games that failed.
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| r.error.is_some()).count()
    }

    fn count(&self, test: impl Fn(&Outcome) -> bool) -> usize {
        self.results
            .iter()
            .filter(|r| r.outcome.as_ref().is_some_and(&test))
            .count()
    }

    /// `3 covers written, 1 game without match, 1 failed`.
    pub fn summary(&self) -> String {
        let mut parts = vec![format!(
            "{} cover{} written",
            self.written(),
            if self.written() == 1 { "" } else { "s" }
        )];
        let unchanged = self.count(|o| o.cover == CoverStatus::Unchanged);
        if unchanged > 0 {
            parts.push(format!("{unchanged} unchanged"));
        }
        let no_art = self.count(|o| o.cover == CoverStatus::NoBoxArt);
        if no_art > 0 {
            parts.push(format!("{no_art} without box art on TheGamesDB"));
        }
        if self.unmatched() > 0 {
            parts.push(format!("{} without match", self.unmatched()));
        }
        if self.failed() > 0 {
            parts.push(format!("{} failed", self.failed()));
        }
        parts.join(", ")
    }
}

/// Errors that end the run: nothing else would succeed either.
fn fatal(error: &Error) -> bool {
    matches!(
        error,
        Error::AllowanceExhausted { .. }
            | Error::InvalidApiKey { .. }
            | Error::NoApiKey
            | Error::Transport { .. }
            | Error::Io { .. }
    )
}

fn now() -> i64 {
    crate::library::unix_seconds(std::time::SystemTime::now())
}

/// The title to search for: the hash database's name when the ROM was known, else the
/// game's name.
pub fn search_title(game: &Game) -> String {
    game.db_name
        .as_deref()
        .map(rom::title_from_file_name)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| game.name.clone())
}

/// The search terms for a title, in order. The No-Intro form `Legend of Zelda, The - A Link
/// to the Past` becomes `The Legend of Zelda: A Link to the Past`; when that has a subtitle,
/// the subtitle alone is the second try, since the API matches substrings.
pub fn search_queries(title: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let title = title.trim();
    if title.is_empty() {
        return queries;
    }
    let mut parts: Vec<String> = title
        .split(" - ")
        .map(str::trim)
        .map(move_article)
        .collect();
    let full = parts.join(": ");
    queries.push(full);
    if parts.len() > 1 {
        let subtitle = parts.remove(1);
        if subtitle.chars().count() >= 4 {
            queries.push(subtitle);
        }
    }
    queries.dedup();
    queries
}

/// `Legend of Zelda, The` → `The Legend of Zelda`.
fn move_article(text: &str) -> String {
    for article in ["The", "A", "An"] {
        if let Some(stem) = text.strip_suffix(&format!(", {article}")) {
            return format!("{article} {}", stem.trim());
        }
    }
    text.to_string()
}

/// Lower case, `&` as `and`, letters and digits only, no leading `the`.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.to_lowercase().replace('&', " and ").chars() {
        if c.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else {
            space = true;
        }
    }
    match out.strip_prefix("the ") {
        Some(rest) => rest.to_string(),
        None => out,
    }
}

/// The search result that fits the title best: an exact title, else an exact alternative
/// title, else a title that starts with the term (or the reverse), else the API's first
/// result (it ranks exact and prefix matches first). Results on other platforms lose ties.
pub fn best_match<'a>(
    games: &'a [GameRecord],
    query: &str,
    system: System,
) -> Option<&'a GameRecord> {
    let wanted = normalise(query);
    let platforms = platform_ids(system);
    let mut best: Option<(u32, &GameRecord)> = None;
    for game in games {
        let title = normalise(&game.title);
        let mut score = if title == wanted {
            100
        } else if game.alternates.iter().any(|a| normalise(a) == wanted) {
            90
        } else if title.starts_with(&wanted) || wanted.starts_with(&title) {
            50
        } else {
            10
        };
        if game.platform.is_none_or(|p| platforms.contains(&p)) {
            score += 5;
        }
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, game));
        }
    }
    best.map(|(_, game)| game)
}

fn candidate(game: &GameRecord) -> Candidate {
    Candidate {
        id: game.id,
        title: game.title.clone(),
        platform: game.platform,
        release_date: game.release_date.clone(),
    }
}

/// `YYYY-MM-DD` with a plausible year; `00` months and days become `01`.
pub fn valid_date(text: &str) -> Option<String> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| {
        text[range.clone()]
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| text[range].parse::<u32>().unwrap_or(0))
    };
    let year = digits(0..4)?;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    if !(1900..=2100).contains(&year) || month > 12 || day > 31 {
        return None;
    }
    Some(format!("{year:04}-{:02}-{:02}", month.max(1), day.max(1)))
}

/// The id → name lists needed to fill metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    /// Publishers.
    pub publishers: BTreeMap<u32, String>,
    /// Developers.
    pub developers: BTreeMap<u32, String>,
}

/// Fills a game's metadata from a record the way hakchi2 CE does. Fields still at their
/// import defaults are filled; others only with `overwrite`. Returns the fields changed.
pub fn apply_metadata(
    game: &mut Game,
    record: &GameRecord,
    names: &Names,
    overwrite: bool,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    let publishers: Vec<&str> = record
        .publisher_ids
        .iter()
        .filter_map(|id| names.publishers.get(id))
        .map(String::as_str)
        .collect();
    let developers: Vec<&str> = record
        .developer_ids
        .iter()
        .filter_map(|id| names.developers.get(id))
        .map(String::as_str)
        .collect();
    if !publishers.is_empty() {
        let publisher = publishers.join(", ").to_uppercase();
        if (overwrite || game.publisher == "UNKNOWN" || game.publisher.is_empty())
            && game.publisher != publisher
        {
            game.publisher = publisher;
            changed.push("publisher");
        }
    } else if !developers.is_empty() && (overwrite || game.copyright.is_empty()) {
        let copyright = match record.release_year() {
            Some(year) => format!("© {year} {}", developers.join(", ")),
            None => format!("© {}", developers.join(", ")),
        };
        if game.copyright != copyright {
            game.copyright = copyright;
            changed.push("copyright");
        }
    }
    if let Some(date) = record.release_date.as_deref().and_then(valid_date)
        && (overwrite || game.release_date == "1900-01-01" || game.release_date.is_empty())
        && game.release_date != date
    {
        game.release_date = date;
        changed.push("release date");
    }
    if let Some(players) = record.players
        && (overwrite || (game.players == 1 && !game.simultaneous))
    {
        let count = u8::try_from(players).unwrap_or(u8::MAX);
        let simultaneous = players == 2 || record.coop == Some(true);
        if game.players != count || game.simultaneous != simultaneous {
            game.players = count;
            game.simultaneous = simultaneous;
            changed.push("players");
        }
    }
    if let Some(overview) = record.overview.as_deref()
        && (overwrite || game.description.is_empty())
        && game.description != overview
    {
        game.description = overview.to_string();
        changed.push("description");
    }
    changed
}

/// Records the user's choice of TheGamesDB game for a game, to be applied by the next
/// [`fetch_art`] (with `force`, or while the game still has placeholder art).
pub fn set_match(library: &Library, code: &str, id: u32) -> Result<()> {
    let game = library.game(code)?;
    library.art_store()?.remember(
        game.crc32,
        IndexEntry {
            tgdb_id: Some(id),
            title: None,
            source: MatchSource::Manual,
            query: None,
            candidates: Vec::new(),
            updated_at: now(),
        },
    )
}

/// Uses a local image file as the cover of a game.
pub fn set_cover_from_file(
    library: &Library,
    code: &str,
    path: &std::path::Path,
    options: &CoverOptions,
) -> Result<crate::images::CoverSet> {
    let mut game = library.game(code)?;
    let bytes = std::fs::read(path).map_err(|e| crate::error::io_error("read", path, e))?;
    library.set_cover(&mut game, &bytes, options)
}

struct Job {
    game: Game,
    id: u32,
    source: MatchSource,
    cached: Option<CachedRecord>,
}

/// Fetches art and metadata for the given games. Per-game problems are reported and the run
/// goes on; an exhausted allowance, a rejected key or a network failure ends it. The index
/// and the allowance are saved either way.
pub async fn fetch_art<T: Transport>(
    library: &Library,
    client: &Client<T>,
    codes: &[String],
    options: &ArtOptions,
    mut progress: impl FnMut(ArtEvent),
) -> Result<ArtReport> {
    let store = library.art_store()?;
    if client.allowance().remaining.is_none() {
        client.limiter().set_allowance(store.allowance()?);
    }
    let mut index = store.index()?;
    let mut report = ArtReport::default();
    let result = run(
        library,
        client,
        &store,
        &mut index,
        codes,
        options,
        &mut progress,
        &mut report,
    )
    .await;
    store.save_index(&index)?;
    let allowance = client.allowance();
    store.save_allowance(&allowance)?;
    report.allowance = allowance;
    report.results.sort_by_key(|r| {
        codes
            .iter()
            .position(|c| *c == r.code)
            .unwrap_or(usize::MAX)
    });
    result.map(|()| report)
}

#[allow(clippy::too_many_arguments)]
async fn run<T: Transport>(
    library: &Library,
    client: &Client<T>,
    store: &ArtStore,
    index: &mut Index,
    codes: &[String],
    options: &ArtOptions,
    progress: &mut impl FnMut(ArtEvent),
    report: &mut ArtReport,
) -> Result<()> {
    // 1. One job per game with a known TheGamesDB id.
    let mut jobs = Vec::new();
    for code in codes {
        let game = match library.game(code) {
            Ok(game) => game,
            Err(error) => {
                progress(ArtEvent::Failed {
                    code: code.clone(),
                    error: error.to_string(),
                });
                report.push_error(code, &error);
                continue;
            }
        };
        if !options.force
            && !game.placeholder_art
            && game.tgdb_id.is_some()
            && options.query.is_none()
        {
            let outcome = Outcome {
                id: game.tgdb_id,
                ..Outcome::new(CoverStatus::Unchanged)
            };
            progress(ArtEvent::Done {
                code: code.clone(),
                outcome: outcome.clone(),
            });
            report.push_outcome(code, outcome);
            continue;
        }
        match resolve(client, store, index, &game, options, progress).await {
            Ok(Some((id, source, title))) => {
                progress(ArtEvent::Matched {
                    code: code.clone(),
                    id,
                    title: title.unwrap_or_default(),
                    source,
                });
                jobs.push(Job {
                    game,
                    id,
                    source,
                    cached: None,
                });
            }
            Ok(None) => {
                let outcome = Outcome::new(CoverStatus::NoMatch);
                progress(ArtEvent::Done {
                    code: code.clone(),
                    outcome: outcome.clone(),
                });
                report.push_outcome(code, outcome);
            }
            Err(error) if fatal(&error) => return Err(error),
            Err(error) => {
                progress(ArtEvent::Failed {
                    code: code.clone(),
                    error: error.to_string(),
                });
                report.push_error(code, &error);
            }
        }
    }

    // 2. Records, from the cache or in batches.
    let mut missing = Vec::new();
    for job in &mut jobs {
        if !options.force {
            job.cached = store.record(job.id)?;
        }
        if job.cached.is_none() && !missing.contains(&job.id) {
            missing.push(job.id);
        }
    }
    if !missing.is_empty() {
        progress(ArtEvent::Fetching {
            code: None,
            what: format!("{} game record(s)", missing.len()),
        });
        let records = client.games(&missing).await?;
        for record in records {
            let cached = CachedRecord {
                record,
                fetched_at: now(),
                front_file: None,
                front_size: None,
                logo: LogoState::Unknown,
            };
            store.save_record(&cached)?;
            for job in jobs.iter_mut().filter(|j| j.id == cached.record.id) {
                job.cached = Some(cached.clone());
            }
        }
    }

    // 3. Publisher and developer names, fetched once and cached.
    let records: Vec<&GameRecord> = jobs
        .iter()
        .filter_map(|j| j.cached.as_ref().map(|c| &c.record))
        .collect();
    let mut names = Names::default();
    if options.metadata {
        let publisher_ids: Vec<u32> = records
            .iter()
            .flat_map(|r| r.publisher_ids.iter().copied())
            .collect();
        let developer_ids: Vec<u32> = records
            .iter()
            .filter(|r| r.publisher_ids.is_empty())
            .flat_map(|r| r.developer_ids.iter().copied())
            .collect();
        names.publishers = ensure_names(
            client,
            store,
            NameList::Publishers,
            &publisher_ids,
            progress,
        )
        .await?;
        names.developers = ensure_names(
            client,
            store,
            NameList::Developers,
            &developer_ids,
            progress,
        )
        .await?;
    }

    // 4. Covers, logos and metadata.
    for job in jobs {
        let code = job.game.code.clone();
        match finish(library, client, store, job, options, &names, progress).await {
            Ok(outcome) => {
                progress(ArtEvent::Done {
                    code: code.clone(),
                    outcome: outcome.clone(),
                });
                report.push_outcome(&code, outcome);
            }
            Err(error) if fatal(&error) => return Err(error),
            Err(error) => {
                progress(ArtEvent::Failed {
                    code: code.clone(),
                    error: error.to_string(),
                });
                report.push_error(&code, &error);
            }
        }
    }
    Ok(())
}

/// The TheGamesDB id of a game: from the index, the hash database or a search. `None` when a
/// search (now or earlier) found nothing.
async fn resolve<T: Transport>(
    client: &Client<T>,
    store: &ArtStore,
    index: &mut Index,
    game: &Game,
    options: &ArtOptions,
    progress: &mut impl FnMut(ArtEvent),
) -> Result<Option<(u32, MatchSource, Option<String>)>> {
    let reusable = index.get(game.crc32).filter(|entry| match entry.source {
        MatchSource::Manual => true,
        MatchSource::HashDb => true,
        MatchSource::Search => !options.research && options.query.is_none(),
    });
    if let Some(entry) = reusable {
        return Ok(entry
            .tgdb_id
            .map(|id| (id, entry.source, entry.title.clone())));
    }
    if options.query.is_none()
        && let Some(&id) = game.tgdb_ids.first()
    {
        index.set(
            game.crc32,
            IndexEntry {
                tgdb_id: Some(id),
                title: None,
                source: MatchSource::HashDb,
                query: None,
                candidates: Vec::new(),
                updated_at: now(),
            },
        );
        return Ok(Some((id, MatchSource::HashDb, None)));
    }
    let queries = match &options.query {
        Some(query) => vec![query.clone()],
        None => search_queries(&search_title(game)),
    };
    let mut tried = Vec::new();
    for query in &queries {
        progress(ArtEvent::Searching {
            code: game.code.clone(),
            query: query.clone(),
        });
        tried.push(query.clone());
        let page = client.search(query, platform_ids(game.system)).await?;
        debug!(
            code = game.code,
            query,
            results = page.games.len(),
            "search"
        );
        if let Some(best) = best_match(&page.games, query, game.system) {
            // The search answer carries the record and its box art: no need to fetch it.
            if options.force || store.record(best.id)?.is_none() {
                store.save_record(&CachedRecord {
                    record: best.clone(),
                    fetched_at: now(),
                    front_file: None,
                    front_size: None,
                    logo: LogoState::Unknown,
                })?;
            }
            let candidates = page
                .games
                .iter()
                .filter(|g| g.id != best.id)
                .take(MAX_CANDIDATES)
                .map(candidate)
                .collect();
            index.set(
                game.crc32,
                IndexEntry {
                    tgdb_id: Some(best.id),
                    title: Some(best.title.clone()),
                    source: MatchSource::Search,
                    query: Some(query.clone()),
                    candidates,
                    updated_at: now(),
                },
            );
            info!(
                code = game.code,
                id = best.id,
                title = best.title,
                "matched by search"
            );
            return Ok(Some((
                best.id,
                MatchSource::Search,
                Some(best.title.clone()),
            )));
        }
    }
    index.set(
        game.crc32,
        IndexEntry {
            tgdb_id: None,
            title: None,
            source: MatchSource::Search,
            query: tried.last().cloned(),
            candidates: Vec::new(),
            updated_at: now(),
        },
    );
    progress(ArtEvent::NoMatch {
        code: game.code.clone(),
        queries: tried,
    });
    Ok(None)
}

/// The cached names, fetched when some needed id is missing. A failed fetch (other than a
/// fatal one) leaves the names empty with a warning.
async fn ensure_names<T: Transport>(
    client: &Client<T>,
    store: &ArtStore,
    list: NameList,
    needed: &[u32],
    progress: &mut impl FnMut(ArtEvent),
) -> Result<BTreeMap<u32, String>> {
    if needed.is_empty() {
        return Ok(BTreeMap::new());
    }
    let cached = store.names(list)?;
    if let Some(cached) = &cached
        && needed.iter().all(|id| cached.contains_key(id))
    {
        return Ok(cached.clone());
    }
    progress(ArtEvent::Fetching {
        code: None,
        what: format!("the {} list", list.key()),
    });
    let fetched = match list {
        NameList::Publishers => client.publishers().await,
        NameList::Developers => client.developers().await,
    };
    match fetched {
        Ok(names) => {
            store.save_names(list, &names)?;
            Ok(names)
        }
        Err(error) if fatal(&error) => Err(error),
        Err(error) => {
            warn!(%error, list = list.key(), "could not fetch the names; metadata stays partial");
            Ok(cached.unwrap_or_default())
        }
    }
}

/// Downloads, resizes and applies everything for one game whose record is known.
async fn finish<T: Transport>(
    library: &Library,
    client: &Client<T>,
    store: &ArtStore,
    job: Job,
    options: &ArtOptions,
    names: &Names,
    progress: &mut impl FnMut(ArtEvent),
) -> Result<Outcome> {
    let Job {
        mut game,
        id,
        source,
        cached,
    } = job;
    let mut cached = cached.ok_or_else(|| Error::Api {
        endpoint: "/v1/Games/ByGameID".to_string(),
        status: 200,
        message: format!("TheGamesDB has no game with id {id}"),
    })?;
    let record = cached.record.clone();
    let mut outcome = Outcome {
        id: Some(id),
        title: Some(record.title.clone()),
        source: Some(source),
        ..Outcome::new(CoverStatus::Unchanged)
    };

    match record.front_boxart() {
        None => outcome.cover = CoverStatus::NoBoxArt,
        Some(front) => {
            let mut applied = false;
            if !options.force
                && let Some(file) = &cached.front_file
                && let Some(bytes) = store.image(id, file)?
            {
                match library.set_cover(&mut game, &bytes, &options.cover) {
                    Ok(set) => {
                        outcome.cover = CoverStatus::Written;
                        outcome.cover_size = Some(set.cover_size);
                        outcome.thumbnail_size = Some(set.thumbnail_size);
                        outcome.from_cache = true;
                        applied = true;
                    }
                    Err(error) => {
                        warn!(code = game.code, id, %error, "cached box art unusable; downloading again");
                        let _ = std::fs::remove_file(store.image_path(id, file));
                    }
                }
            }
            if !applied {
                progress(ArtEvent::Fetching {
                    code: Some(game.code.clone()),
                    what: format!("{} box art", options.size),
                });
                let mut downloaded = client
                    .download(&front.url(&record.image_urls, options.size))
                    .await?;
                if downloaded.is_none() && options.size != ArtSize::Original {
                    warn!(code = game.code, id, size = %options.size, "no box art in this size; trying the original");
                    downloaded = client
                        .download(&front.url(&record.image_urls, ArtSize::Original))
                        .await?;
                }
                match downloaded {
                    Some(data) => {
                        // Decoding first keeps broken downloads out of the cache.
                        let set = library.set_cover(&mut game, &data, &options.cover)?;
                        let name = front_file_name(front);
                        store.save_image(id, &name, &data)?;
                        cached.front_file = Some(name);
                        cached.front_size = Some(options.size);
                        store.save_record(&cached)?;
                        outcome.cover = CoverStatus::Written;
                        outcome.cover_size = Some(set.cover_size);
                        outcome.thumbnail_size = Some(set.thumbnail_size);
                    }
                    None => outcome.cover = CoverStatus::NoBoxArt,
                }
            }
        }
    }

    if options.logo && (options.force || cached.logo == LogoState::Unknown) {
        progress(ArtEvent::Fetching {
            code: Some(game.code.clone()),
            what: "clear logo".to_string(),
        });
        match client.download(&clear_logo_url(id)).await {
            Ok(Some(data)) => {
                store.save_image(id, CLEAR_LOGO_FILE, &data)?;
                cached.logo = LogoState::Present;
            }
            Ok(None) => cached.logo = LogoState::Missing,
            Err(error) if fatal(&error) => return Err(error),
            Err(error) => warn!(code = game.code, %error, "clear logo not downloaded"),
        }
        store.save_record(&cached)?;
    }
    outcome.logo = cached.logo;

    if options.metadata {
        outcome.metadata = apply_metadata(&mut game, &record, names, options.overwrite);
    }
    game.tgdb_id = Some(id);
    library.save(&game)?;
    info!(code = game.code, id, cover = ?outcome.cover, metadata = ?outcome.metadata, "art applied");
    Ok(outcome)
}

/// `front.jpg`: the cached file name of a front box art.
fn front_file_name(front: &ArtImage) -> String {
    let extension = front
        .filename
        .rsplit('.')
        .next()
        .filter(|ext| ext.len() <= 4 && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or("img")
        .to_ascii_lowercase();
    format!("front.{extension}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{
        client::testing::{Scripted, client as test_client, fixture},
        *,
    };
    use crate::{
        HashDb,
        images::{png_dimensions, tests::gradient},
        rom::{self, snes::RomType, tests::synthetic_snes},
    };

    const SEARCH_URL: &str = "https://api.thegamesdb.net/v1/Games/ByGameName?apikey=test-key&name=Chrono%20Trigger&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform&filter[platform]=6";
    const BY_ID_URL: &str = "https://api.thegamesdb.net/v1/Games/ByGameID?apikey=test-key&id=1255&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform";
    const FRONT_URL: &str = "https://cdn.thegamesdb.net/images/medium/boxart/front/1255-1.jpg";
    const LOGO_URL: &str = "https://cdn.thegamesdb.net/images/original/clearlogo/1255.png";
    const PUBLISHERS_URL: &str = "https://api.thegamesdb.net/v1/Publishers?apikey=test-key";

    /// A library with Chrono Trigger: known to the hash database (`with_db`) or not.
    fn library(with_db: bool) -> (tempfile::TempDir, Library, Game) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("lib")).unwrap();
        let rom = synthetic_snes("CHRONO TRIGGER", RomType::HiRom, 0x02, 0x03);
        let db = HashDb::parse(&format!(
            r#"<romfiles><file name="Chrono Trigger (USA).sfc" crc32="{:08X}"><tgdb>1255</tgdb></file></romfiles>"#,
            rom::crc32(&rom)
        ))
        .unwrap();
        let imported = library
            .import_bytes("Chrono Trigger (USA).sfc", rom, with_db.then_some(&db))
            .unwrap();
        (dir, library, imported.game)
    }

    fn events() -> (
        std::sync::Arc<std::sync::Mutex<Vec<ArtEvent>>>,
        impl FnMut(ArtEvent),
    ) {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = log.clone();
        (log, move |event| sink.lock().unwrap().push(event))
    }

    #[test]
    fn queries_follow_no_intro_names() {
        assert_eq!(search_queries("Chrono Trigger"), ["Chrono Trigger"]);
        assert_eq!(
            search_queries("Legend of Zelda, The - A Link to the Past"),
            [
                "The Legend of Zelda: A Link to the Past",
                "A Link to the Past"
            ]
        );
        assert_eq!(search_queries("Mega Man X - 2"), ["Mega Man X: 2"]);
        assert_eq!(search_queries("Link, A"), ["A Link"]);
        assert!(search_queries("  ").is_empty());
        assert_eq!(
            normalise("The Legend of Zelda: A Link to the Past"),
            "legend of zelda a link to the past"
        );
        assert_eq!(
            normalise("Kirby's Dream Land 3 & more!"),
            "kirby s dream land 3 and more"
        );
        let mut game = crate::game::tests::chrono_trigger();
        assert_eq!(search_title(&game), "Chrono Trigger");
        game.db_name = None;
        game.name = "Edited".into();
        assert_eq!(search_title(&game), "Edited");
    }

    #[test]
    fn best_match_prefers_exact_titles_and_the_platform() {
        let value: serde_json::Value =
            serde_json::from_str(fixture("by-game-name-chrono-trigger")).unwrap();
        let page = client::parse_games_page(&value);
        assert_eq!(
            best_match(&page.games, "Chrono Trigger", System::Snes)
                .unwrap()
                .id,
            1255
        );
        assert_eq!(
            best_match(&page.games, "Chrono Trigger (JP)", System::Snes)
                .unwrap()
                .id,
            1255
        );
        assert_eq!(
            best_match(&page.games, "Chrono", System::Snes).unwrap().id,
            90210,
            "prefix: first in API order"
        );
        assert_eq!(
            best_match(&page.games, "Something else", System::Snes)
                .unwrap()
                .id,
            90210,
            "fallback: the API's first result"
        );
        let mut games = page.games.clone();
        games[0].platform = Some(7);
        assert_eq!(
            best_match(&games, "Something else", System::Snes)
                .unwrap()
                .id,
            1255,
            "other platforms lose ties"
        );
        assert!(best_match(&[], "x", System::Snes).is_none());
    }

    #[test]
    fn dates_are_checked() {
        assert_eq!(valid_date("1995-03-11").as_deref(), Some("1995-03-11"));
        assert_eq!(valid_date("1995-00-00").as_deref(), Some("1995-01-01"));
        assert_eq!(valid_date("1995-3-11"), None);
        assert_eq!(valid_date("0000-01-01"), None);
        assert_eq!(valid_date("1995-13-01"), None);
        assert_eq!(valid_date(""), None);
    }

    #[test]
    fn metadata_fills_defaults_only_unless_overwriting() {
        let value: serde_json::Value = serde_json::from_str(fixture("by-game-id-1255")).unwrap();
        let record = client::parse_games_page(&value).games.remove(0);
        let names = Names {
            publishers: [(4365, "Square".to_string())].into(),
            developers: [(1428, "Square".to_string())].into(),
        };
        let mut game = crate::game::tests::chrono_trigger();
        game.publisher = "UNKNOWN".into();
        game.release_date = "1900-01-01".into();
        let changed = apply_metadata(&mut game, &record, &names, false);
        assert_eq!(changed, ["publisher", "release date", "description"]);
        assert_eq!(game.publisher, "SQUARE");
        assert_eq!(game.release_date, "1995-03-11");
        assert_eq!((game.players, game.simultaneous), (1, false));
        assert!(game.description.starts_with("Crono"));
        assert!(
            apply_metadata(&mut game, &record, &names, false).is_empty(),
            "a second pass changes nothing"
        );

        // Edited fields are kept without overwrite.
        game.publisher = "Mine".into();
        game.release_date = "2001-01-01".into();
        assert!(apply_metadata(&mut game, &record, &names, false).is_empty());
        assert_eq!(
            apply_metadata(&mut game, &record, &names, true),
            ["publisher", "release date"]
        );
        assert_eq!(game.publisher, "SQUARE");

        // Two players means simultaneous; no publisher gives a copyright line from developers.
        let mut record = record;
        record.players = Some(2);
        record.publisher_ids.clear();
        let mut game = crate::game::tests::chrono_trigger();
        game.publisher = "UNKNOWN".into();
        assert_eq!(
            apply_metadata(&mut game, &record, &names, false),
            ["copyright", "players", "description"]
        );
        assert_eq!(game.copyright, "© 1995 Square");
        assert_eq!((game.players, game.simultaneous), (2, true));
        assert_eq!(game.publisher, "UNKNOWN");
        record.release_date = None;
        record.coop = Some(true);
        record.players = Some(4);
        let mut game = crate::game::tests::chrono_trigger();
        game.copyright.clear();
        apply_metadata(&mut game, &record, &names, true);
        assert_eq!(game.copyright, "© Square");
        assert_eq!((game.players, game.simultaneous), (4, true));
        assert!(!names.publishers.is_empty());
    }

    #[tokio::test]
    async fn hash_db_ids_skip_the_search_and_everything_is_cached() {
        let (_dir, library, game) = library(true);
        assert_eq!(game.tgdb_ids, [1255]);
        let front = gradient(600, 400, image::ImageFormat::Jpeg);
        let logo = gradient(400, 141, image::ImageFormat::Png);
        let scripted = Scripted::default()
            .answer(BY_ID_URL, 200, fixture("by-game-id-1255"))
            .answer(FRONT_URL, 200, front.clone())
            .answer(LOGO_URL, 200, logo)
            .answer(PUBLISHERS_URL, 200, fixture("publishers"));
        let client = test_client(scripted);
        let (log, progress) = events();
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions::default(),
            progress,
        )
        .await
        .unwrap();
        assert_eq!(report.results.len(), 1);
        let outcome = report.results[0].outcome.clone().unwrap();
        assert_eq!(outcome.id, Some(1255));
        assert_eq!(outcome.title.as_deref(), Some("Chrono Trigger"));
        assert_eq!(outcome.source, Some(MatchSource::HashDb));
        assert_eq!(outcome.cover, CoverStatus::Written);
        assert_eq!(outcome.cover_size, Some((228, 152)));
        assert_eq!(outcome.thumbnail_size, Some((40, 28)));
        assert!(!outcome.from_cache);
        assert_eq!(outcome.logo, LogoState::Present);
        assert_eq!(
            outcome.metadata,
            ["publisher", "release date", "description"]
        );
        assert_eq!(report.summary(), "1 cover written");
        assert_eq!(report.allowance.remaining, Some(2946));
        assert_eq!(
            client.transport().requests(),
            [BY_ID_URL, PUBLISHERS_URL, FRONT_URL, LOGO_URL]
        );
        assert!(matches!(
            &log.lock().unwrap()[0],
            ArtEvent::Matched {
                id: 1255,
                source: MatchSource::HashDb,
                ..
            }
        ));

        // The game on disk.
        let saved = library.game(&game.code).unwrap();
        assert!(!saved.placeholder_art);
        assert_eq!(saved.tgdb_id, Some(1255));
        assert_eq!(saved.publisher, "SQUARE");
        assert_eq!(saved.release_date, "1995-03-11");
        assert!(saved.description.starts_with("Crono"));
        let cover = std::fs::read(library.game_dir(&game.code).join(game.cover_file())).unwrap();
        assert_eq!(png_dimensions(&cover), Some((228, 152)));
        let thumb =
            std::fs::read(library.game_dir(&game.code).join(game.thumbnail_file())).unwrap();
        assert_eq!(png_dimensions(&thumb), Some((40, 28)));
        let desktop =
            std::fs::read_to_string(library.game_dir(&game.code).join(game.desktop_file()))
                .unwrap();
        assert!(desktop.contains("Publisher=SQUARE\n"));
        assert!(desktop.contains("ReleaseDate=1995-03-11\n"));
        assert!(desktop.contains("Text = \nCrono"));

        // The store.
        let store = library.art_store().unwrap();
        let entry = store.lookup(game.crc32).unwrap().unwrap();
        assert_eq!(
            (entry.tgdb_id, entry.source),
            (Some(1255), MatchSource::HashDb)
        );
        let cached = store.record(1255).unwrap().unwrap();
        assert_eq!(cached.front_file.as_deref(), Some("front.jpg"));
        assert_eq!(cached.front_size, Some(ArtSize::Medium));
        assert_eq!(cached.logo, LogoState::Present);
        assert_eq!(store.image(1255, "front.jpg").unwrap(), Some(front));
        assert!(store.image(1255, CLEAR_LOGO_FILE).unwrap().is_some());
        assert_eq!(
            store.names(NameList::Publishers).unwrap().unwrap()[&4365],
            "Square"
        );
        assert_eq!(store.allowance().unwrap().remaining, Some(2946));

        // A second run touches nothing.
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions::default(),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            report.results[0].outcome.as_ref().unwrap().cover,
            CoverStatus::Unchanged
        );
        assert_eq!(client.transport().requests().len(), 4);

        // Re-importing the same ROM gives placeholder art again; applying uses the cache only.
        let rom = synthetic_snes("CHRONO TRIGGER", RomType::HiRom, 0x02, 0x03);
        let again = library
            .import_bytes("Chrono Trigger (USA).sfc", rom, None)
            .unwrap();
        assert!(again.game.placeholder_art && again.game.tgdb_ids.is_empty());
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions::default(),
            |_| {},
        )
        .await
        .unwrap();
        let outcome = report.results[0].outcome.clone().unwrap();
        assert_eq!(outcome.cover, CoverStatus::Written);
        assert!(outcome.from_cache);
        assert_eq!(outcome.source, Some(MatchSource::HashDb), "from the index");
        assert_eq!(client.transport().requests().len(), 4, "no network at all");
        assert_eq!(library.game(&game.code).unwrap().publisher, "SQUARE");

        // Force fetches everything again.
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions {
                force: true,
                ..ArtOptions::default()
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            report.results[0].outcome.as_ref().unwrap().cover,
            CoverStatus::Written
        );
        assert_eq!(
            client.transport().requests().len(),
            7,
            "record, box art and logo again"
        );
    }

    #[tokio::test]
    async fn unknown_roms_are_searched_once() {
        let (_dir, library, game) = library(false);
        assert!(game.tgdb_ids.is_empty());
        assert_eq!(game.name, "Chrono Trigger");
        let scripted = Scripted::default()
            .answer(SEARCH_URL, 200, fixture("by-game-name-chrono-trigger"))
            .answer(FRONT_URL, 200, gradient(300, 420, image::ImageFormat::Jpeg))
            .answer(PUBLISHERS_URL, 200, fixture("publishers"));
        let client = test_client(scripted);
        let (log, progress) = events();
        let options = ArtOptions {
            logo: false,
            ..ArtOptions::default()
        };
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &options,
            progress,
        )
        .await
        .unwrap();
        let outcome = report.results[0].outcome.clone().unwrap();
        assert_eq!(outcome.id, Some(1255));
        assert_eq!(outcome.source, Some(MatchSource::Search));
        assert_eq!(outcome.cover, CoverStatus::Written);
        assert_eq!(outcome.cover_size, Some((146, 204)));
        assert_eq!(outcome.logo, LogoState::Unknown);
        assert_eq!(
            client.transport().requests(),
            [SEARCH_URL, PUBLISHERS_URL, FRONT_URL],
            "the search answer serves as the record"
        );
        let events = log.lock().unwrap().clone();
        assert!(
            matches!(&events[0], ArtEvent::Searching { query, .. } if query == "Chrono Trigger")
        );
        assert!(
            matches!(&events[1], ArtEvent::Matched { id: 1255, title, .. } if title == "Chrono Trigger")
        );
        let entry = library
            .art_store()
            .unwrap()
            .lookup(game.crc32)
            .unwrap()
            .unwrap();
        assert_eq!(entry.query.as_deref(), Some("Chrono Trigger"));
        assert_eq!(
            entry.candidates.iter().map(|c| c.id).collect::<Vec<_>>(),
            [90210, 1256]
        );

        // No match: remembered, not searched again; `research` searches again.
        let rom = synthetic_snes("NOTHING", RomType::LoRom, 0x00, 0x00);
        let nothing = library.import_bytes("Nothing.sfc", rom, None).unwrap().game;
        let no_match_url = "https://api.thegamesdb.net/v1/Games/ByGameName?apikey=test-key&name=Nothing&fields=players%2Cpublishers%2Coverview%2Cplatform%2Ccoop%2Calternates&include=boxart%2Cplatform&filter[platform]=6";
        let scripted = Scripted::default().answer(no_match_url, 200, fixture("by-game-name-empty"));
        let client = test_client(scripted);
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&nothing.code),
            &options,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            report.results[0].outcome.as_ref().unwrap().cover,
            CoverStatus::NoMatch
        );
        assert_eq!(report.summary(), "0 covers written, 1 without match");
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&nothing.code),
            &options,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            report.results[0].outcome.as_ref().unwrap().cover,
            CoverStatus::NoMatch
        );
        assert_eq!(client.transport().requests().len(), 1);
        let entry = library
            .art_store()
            .unwrap()
            .lookup(nothing.crc32)
            .unwrap()
            .unwrap();
        assert_eq!(
            (entry.tgdb_id, entry.query.as_deref()),
            (None, Some("Nothing"))
        );
        fetch_art(
            &library,
            &client,
            std::slice::from_ref(&nothing.code),
            &ArtOptions {
                research: true,
                ..options.clone()
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(client.transport().requests().len(), 2);
        assert!(library.game(&nothing.code).unwrap().placeholder_art);

        // A manual choice is applied on the next run.
        set_match(&library, &nothing.code, 1255).unwrap();
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&nothing.code),
            &options,
            |_| {},
        )
        .await
        .unwrap();
        let outcome = report.results[0].outcome.clone().unwrap();
        assert_eq!(
            (outcome.id, outcome.source),
            (Some(1255), Some(MatchSource::Manual))
        );
        assert_eq!(outcome.cover, CoverStatus::Written);
        assert!(
            outcome.from_cache,
            "the record and the box art were cached by the first game"
        );
        assert_eq!(client.transport().requests().len(), 2);
    }

    #[tokio::test]
    async fn an_exhausted_allowance_ends_the_run_and_is_remembered() {
        let (_dir, library, game) = library(true);
        let scripted = Scripted::default().answer(BY_ID_URL, 403, fixture("error-403-exhausted"));
        let client = test_client(scripted);
        let error = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions::default(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            Error::AllowanceExhausted {
                refresh_in_secs: Some(_)
            }
        ));
        let store = library.art_store().unwrap();
        assert_eq!(store.allowance().unwrap().remaining, Some(0));
        assert!(
            store.lookup(game.crc32).unwrap().is_some(),
            "the index is saved anyway"
        );
        // A new client restores the allowance from the store and does not even try.
        let client = test_client(Scripted::default());
        let error = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions::default(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::AllowanceExhausted { .. }));
        assert!(client.transport().requests().is_empty());
        assert!(library.game(&game.code).unwrap().placeholder_art);
    }

    #[tokio::test]
    async fn per_game_problems_do_not_stop_the_run() {
        let (_dir, library, game) = library(true);
        // The box art download answers 404 in medium and original, and the publishers list fails.
        let scripted = Scripted::default()
            .answer(BY_ID_URL, 200, fixture("by-game-id-1255"))
            .answer(PUBLISHERS_URL, 500, "{\"code\": 500, \"status\": \"down\"}")
            .answer(LOGO_URL, 404, "");
        let client = test_client(scripted);
        let codes = [
            game.code.clone(),
            "CLV-U-ZZZZZ".to_string(),
            "bad".to_string(),
        ];
        let report = fetch_art(&library, &client, &codes, &ArtOptions::default(), |_| {})
            .await
            .unwrap();
        assert_eq!(report.results.len(), 3);
        let outcome = report.results[0].outcome.clone().unwrap();
        assert_eq!(outcome.cover, CoverStatus::NoBoxArt);
        assert_eq!(outcome.logo, LogoState::Missing);
        assert_eq!(
            outcome.metadata,
            ["release date", "description"],
            "no publisher names"
        );
        assert!(
            report.results[1]
                .error
                .as_deref()
                .unwrap()
                .contains("no game CLV-U-ZZZZZ")
        );
        assert!(
            report.results[2]
                .error
                .as_deref()
                .unwrap()
                .contains("not a game code")
        );
        assert_eq!(
            report.summary(),
            "0 covers written, 1 without box art on TheGamesDB, 2 failed"
        );
        let saved = library.game(&game.code).unwrap();
        assert!(saved.placeholder_art);
        assert_eq!(saved.tgdb_id, Some(1255));
        assert_eq!(saved.release_date, "1995-03-11");
        // A broken image is a per-game failure too.
        let scripted = Scripted::default().answer(FRONT_URL, 200, "not an image");
        let client = test_client(scripted);
        let report = fetch_art(
            &library,
            &client,
            std::slice::from_ref(&game.code),
            &ArtOptions {
                force: false,
                ..ArtOptions::default()
            },
            |_| {},
        )
        .await
        .unwrap();
        assert!(report.results[0].outcome.is_none());
        let error = report.results[0].error.clone().unwrap();
        assert!(error.starts_with("image:"), "{error}");
        assert!(
            library
                .art_store()
                .unwrap()
                .image(1255, "front.jpg")
                .unwrap()
                .is_none(),
            "nothing broken is cached"
        );
    }

    #[test]
    fn local_images_become_covers() {
        let (_dir, library, game) = library(false);
        let path = _dir.path().join("cover.png");
        std::fs::write(&path, gradient(100, 100, image::ImageFormat::Png)).unwrap();
        let set =
            set_cover_from_file(&library, &game.code, &path, &CoverOptions::default()).unwrap();
        assert_eq!(set.cover_size, (204, 204));
        let saved = library.game(&game.code).unwrap();
        assert!(!saved.placeholder_art);
        assert_eq!(saved.tgdb_id, None);
        let mut saved = saved;
        library.clear_cover(&mut saved).unwrap();
        assert!(library.game(&game.code).unwrap().placeholder_art);
        assert!(
            set_cover_from_file(&library, "CLV-U-ZZZZZ", &path, &CoverOptions::default()).is_err()
        );
    }
}
