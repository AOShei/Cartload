# Task 07: cover art and metadata from TheGamesDB

Depends on task 06. Google image scraping is dead; do not implement it.

- `library::art` module: a TheGamesDB v1 client (`https://api.thegamesdb.net/v1`) with the API
  key read from an environment variable `CARTLOAD_TGDB_KEY` or the app config, never hard-coded.
  Search by name and platform (SNES, NES, Mega Drive platform IDs), fetch boxart and clear logo
  URLs, download and resize to the sizes the console needs (check hakchi2 CE's `NesApplication`
  image handling for the exact dimensions and the thumbnail rules).
- A local index mapping ROM hash → TheGamesDB game ID, persisted in the library, so a second
  import of the same ROM needs no search.
- A rate limiter respecting the API's monthly allowance headers.
- CLI: `cartload-library art <game-id>` and `art --all`.
- Tests with recorded JSON fixtures; no network in tests.

Verify and commit.
