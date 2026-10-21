# Report: 07-art

## What was done

`cartload-library` gained the `art` module (TheGamesDB v1), real cover processing in
`images`, and the `art` / `key` CLI commands. No Google scraping. Layout:

- `src/art/client.rs`: `Client<T: Transport>` over `https://api.thegamesdb.net/v1` with the
  key as a query parameter, never logged or shown (`ApiKey` hides itself in `Debug`/`Display`,
  `redact()` strips it from URLs in errors). `ApiKey::resolve(explicit, stored)`: `--key`, then
  the `CARTLOAD_TGDB_KEY` variable, then `library.json`'s `tgdb_key`; nothing built in.
  Endpoints: `search(name, platforms)` (`Games/ByGameName` with `filter[platform]`,
  `include=boxart,platform`), `games(ids)` (`Games/ByGameID`, chunks of 20, follows
  `pages.next`), `images(ids, kinds)` (`Games/Images`), `publishers()`, `developers()`,
  `download(url)` (CDN, no allowance; `None` on 404). Platform ids verified on
  thegamesdb.net: SNES 6, NES 7, Sega Genesis 18, Sega Mega Drive 36 (Mega Drive searches
  send `18,36`). Responses are parsed from `serde_json::Value` with PHP's quirks tolerated
  (numbers as strings, empty maps as `[]`, `data.games` as array or object). `ArtSize`
  (`original|small|thumb|cropped_center_thumb|medium|large`); measured on game 1255's front
  box art: 2100×1533, 1479×1080, 986×720, 410×300, 205×150, so `medium` is the default for
  the 228×204 cover. Clear logo URL as hakchi2 CE: `images/original/clearlogo/<id>.png`.
- Rate limiter (`Limiter`, same file): the API reports the allowance in the JSON body, not in
  headers (`remaining_monthly_allowance`, `extra_allowance`, `allowance_refresh_timer`;
  checked against the API's GPL source, `API/include/middleware.php`). Every answer updates
  `Allowance { remaining, extra, refresh_at, updated_at }`; once remaining and extra are 0 the
  limiter refuses to send until the reset time (one day when the key has no timer) with
  `Error::AllowanceExhausted { refresh_in_secs }`; a 403 with a positive or `null` timer is
  "used up", a 403/401 with timer 0 is `Error::InvalidApiKey` (the live API answers
  `403 … "allowance_refresh_timer":0` for a bad key; recorded in `error-403-invalid-key.json`).
  Requests are spaced 250 ms apart. The allowance is persisted in `art/allowance.json` and
  restored into a new client, so an exhausted key is not retried across runs.
- `src/art/index.rs`: `ArtStore` at `<library>/art/`: `index.json` (CRC-32 hex → `IndexEntry
  { tgdb_id (None = searched, nothing found), title, source: hash_db|search|manual, query,
  candidates (up to 5 other results), updated_at }`), `tgdb/<id>/record.json` (the
  `GameRecord` plus `front_file`, `front_size`, `logo: unknown|missing|present`), the
  downloads `front.<ext>` and `clearlogo.png`, `names/publishers.json` and
  `names/developers.json`. JSON writes go through a temp file and rename (also for
  `game.json` and `library.json` now).
- `src/art/mod.rs`: `fetch_art(library, client, codes, options, progress)`: per game, the id
  from the index, else the hash database's first id, else searches (`search_queries`: the
  No-Intro `Legend of Zelda, The - A Link to the Past` becomes `The Legend of Zelda: A Link
  to the Past`, then the subtitle alone, since the API matches substrings; `best_match`:
  exact title, exact alternate, prefix, else the API's first result, other platforms lose
  ties); the search answer is cached as the record, so no `ByGameID` call follows. Records
  are fetched in batches of 20 for the rest; publisher/developer lists once and cached.
  Then the front box art (cached file, else `medium`, else `original`), decoded before it is
  cached, `Library::set_cover` writes `CLV-U-XXXXX.png` and `_small.png`; the clear logo goes
  to the cache only (used for nothing on NES/SNES; CE needs it for Mega Drive spines).
  Metadata as CE's `ScrapeGames`: publisher names joined and upper-cased, `© year developers`
  as copyright when there is no publisher, release date (`00` parts become `01`), players with
  2 = simultaneous (or `coop`), description from `overview`. Only fields still at their import
  defaults are filled; `overwrite` replaces. `Game` gained `tgdb_id` and `description` (the
  latter now rendered in the launch entry). `ArtOptions { size, cover, force, research, query,
  metadata, overwrite, logo }`, `ArtEvent` progress (serialisable, for the UI), `ArtReport`
  with per-game `Outcome` and `summary()`. Fatal errors (allowance, key, transport, I/O) end
  the run; others are per-game failures. `set_match(library, code, id)` (manual choice) and
  `set_cover_from_file` (local image, no network).
- `src/images.rs`: `make_cover(bytes, CoverOptions)` with hakchi2 CE's rules
  (`NesMenuElementBase.SetImage`, `Shared.ResizeImage`): cover fitted into 228×204 keeping
  proportions and upscaling, thumbnail into 40×40 without upscaling, odd sizes rounded up to
  even, optional centred thumbnail (`CenterThumbnail`), Catmull-Rom resampling (`image`
  crate, png+jpeg+gif+bmp only), and CE's `CompressCover` default: 8-bit palette PNGs (exact
  palette when ≤ 256 colours, NeuQuant otherwise, tRNS for transparency).
- CLI: `cartload-library art <CODE>... | --all [--key] [--force] [--research] [--search NAME]
  [--id N] [--image FILE] [--size] [--no-metadata] [--overwrite] [--no-logo]
  [--center-thumbnail] [--no-quantize] [--json]` and `key [--set KEY | --clear]`; `list` shows
  the resolved id and `no art`.
- Fixtures in `tests/fixtures/tgdb/`: `by-game-name-*`, `by-game-id-*` (two pages),
  `images-1255`, `publishers`, `developers`, `error-401`, `error-403-exhausted`,
  `error-403-invalid-key`. All but the last are hand-written to the API's OpenAPI spec and PHP
  source (no key was available to record real answers); the last is a live recording.

## Verified

- `cargo test -p cartload-library`: 84 tests, no network (a scripted `Transport` answers from
  the fixtures and logs every URL). Covered: key resolution and redaction, URL encoding,
  allowance parsing (strings, missing `extra_allowance`, null timer), limiter refusal and
  spacing, search/record/images/names parsing, paging and 20-id batching, error mapping,
  index/record/names/allowance persistence, CE's fit sizes (2100×1533 → 228×166, portrait →
  146×204, 40×40 rules), palette PNGs smaller than RGB and lossless when ≤ 256 colours,
  centred thumbnails, the full flow with a hash-db id (4 requests, then 0 on a second run,
  0 after re-importing the same ROM, 7 with `--force`), the search flow (search answer used
  as record, candidates kept, no-match remembered and not repeated, `research`, manual
  choice applied from the cache), exhausted allowance ending the run and persisting, per-game
  failures (missing box art, failed names list, broken image not cached) not stopping it,
  metadata fill-vs-overwrite rules, local image covers.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo doc -p cartload-library --no-deps`: clean.
- CLI on a scratch library (`CARTLOAD_LIBRARY=/tmp/cartload-smoke/lib`) with
  `~/Documents/Games/SNES/ActRaiser.sfc` (read only): `import` → `CLV-U-DDSYO`, hash db
  `tgdb [1046]`; `key` → "comes from nowhere"; `art --all` without a key → the `NoApiKey`
  message; `art CLV-U-DDSYO --key not-a-real-key` → one live request, "TheGamesDB rejected
  the API key (HTTP 403: Invalid API key was provided.)", `allowance.json` left unknown,
  `index.json` holds the hash-db entry; `art CLV-U-DDSYO --image original.jpg` (the real
  2100×1533 CDN box art of game 1255) → `cover 228x166 and thumbnail 40x30`, both
  `8-bit colormap` PNGs; `key --set abc` / `--clear` round-trip in `library.json`.

## Decisions

- The clear logo is fetched from the CDN by convention (as CE) and kept in the cache, not
  in the game directory, so it is not uploaded to the console. `Client::images()` exists for
  the UI to list alternative covers.
- Game names are not replaced by TheGamesDB titles (the hash-db title is already applied at
  import and the user may have edited the name). The title is kept in the index and report.
- `--all` takes games with placeholder art or no resolved id; `--force` takes every game and
  ignores the cache; `--research` repeats searches but keeps manual choices.
- Genres are not mapped (no field on NES/SNES entries). Mega Drive games cannot be imported
  yet (task 06), so the 18/36 platform filter is untested against real searches.
- Downloaded box art is kept at the chosen size (~170 KB per game at `medium`) so re-applying
  needs no network; `--size thumb` keeps it to ~30 KB.

## Left or uncertain

- No run against the live API with a valid key: the request/response shapes follow the
  OpenAPI spec and the PHP source and CE's models, but a real `ByGameName`/`ByGameID`
  answer should be checked once a key is at hand (`cartload-library art --all -v`).
- The search heuristics (`search_queries`, `best_match`) are untested against real data.
- The driver's `.agent/run.py` edit and `.agent/tasks/*` moves stay uncommitted.
