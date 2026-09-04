# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.22.1] - 2026-09-04

### Fixed
- **A cycle in the server's folder data can no longer hang pcli2** - Every walk over the folder hierarchy (path building, `folder list --format tree`, subtree filtering) now remembers where it has been, so a folder whose parent chain loops back on itself (or lists itself as its parent) ends the walk with a warning instead of looping forever or overflowing the stack. The "did you mean" suggestions build the path of every folder, so one bad record used to hang every not-found message.

## [1.22.0] - 2026-09-04

### Fixed
- **The new-version check no longer delays exit** - It ran after the command finished and could hold the prompt for up to three seconds on a stale cache. It now starts before the command and is only *reported* at exit; a lookup still in flight is abandoned rather than waited for.
- **Match percentages are formatted the same way in every CSV** - Single-asset match CSV printed `100.0` where the folder report and the `--metadata` CSV printed `100`.
- **`folder visual-match` says when an asset hit `--limit`** - The per-asset cap (default 100) truncated silently in folder mode; text match already warned.
- **`folder create` prints the new folder's UUID** - It printed nothing, so a script had to resolve the path it had just created.

### Changed
- **Root `--help` (and the man page) list every environment variable** pcli2 reads; the man page previously mentioned only `PCLI2_NO_COLOR`.
- **Folder match commands document their row order** in `--help`: pair order for CSV and JSON, match percentage for Excel.
- Help text: "Folder UUID" instead of "Resource's folder UUID"; the `--output` help on `folder geometric-match` no longer contradicts itself.

## [1.21.0] - 2026-09-04

### Fixed
- **The active tenant follows the environment** - One tenant selection was shared by every environment, so `env use staging` left production's tenant UUID active and every command failed with "Tenant not found" until `tenant use` was run again. Each environment now remembers its own tenant (older versions still read the top-level value). Folder and metadata caches are keyed by environment too, so a staging tenant cloned from production with the same UUID can no longer be served production's folder tree.
- **Per-asset failure messages survive `--progress`** - During a folder match the reasons for individual failures, and the "stopping after repeated authentication failures" explanation, were written underneath the redrawing progress display and painted over (the same defect fixed for the scan spinner in 1.18.3). They are now printed with the display lifted.
- **`env reset`, `env remove` and overwriting with `env add` ask first** - `env reset` wiped every environment with no prompt; `env add` silently replaced an existing one. All three honour the global `--yes`, and refuse rather than assume when no prompt can be shown.
- **`env add` rejects a URL that is not one** - `--api-url 'not a url'` was saved and surfaced later as an opaque request error.
- **`env remove --name nope` says the environment does not exist** (exit 67) instead of "missing value for property".
- **`--format JSON` works** - The value is matched case-insensitively, as `PCLI2_FORMAT` already was.
- **`asset list --folder-uuid`** - Every other folder command accepted a UUID; this one only took a path.
- **`user list` shows its `ls` alias in help** like every other list command.
- **`--verbose` shows pcli2's own debug output** without the HTTP stack's; `RUST_LOG` still opens everything.
- **Folder trees and lists sort case-insensitively** ("alpha" before "Zeta").
- **Configuration and formatting errors show the cause as a sentence** rather than a Rust debug dump.
- **`asset delete` and `folder delete` work again** - The request-path rewrite in this release cycle sent the relative API path as the whole URL for DELETE; caught before release, with a regression test.
- **`asset metadata create-batch` fetches the field registry once and lists each parent folder once** - Both happened per row: a 5,000-row batch into a 10,000-asset folder was about a quarter of a million requests. A registry fetch that fails is now an error rather than "no fields exist", which used to turn every value into a new text field and report the API's rejection as a type conflict in the user's CSV.
- **Cache files are written atomically and carry a schema version** - Two pcli2 processes (a script under `xargs -P`) could read a half-written cache and refetch the whole hierarchy; a cache written by another version is now discarded instead of decoded wrongly. A cache timestamp from the future no longer panics a debug build.
- **Asset listing stops if the server echoes a stale page number** - The page loop had no guard against being asked for the same page forever.
- **Listings that hit the safety page cap say so** - The truncation of very large user and asset listings was logged at debug level only.
- **`--files a,b,c` rejects a path that does not exist** - It was dropped silently, so a typo uploaded one file fewer. Directories matched by a glob are skipped and counted instead of becoming per-file failures.
- **`asset metadata create --type number` no longer saturates large values** - `1e20` was stored as 9223372036854775807. `--type boolean` with a value that is not a boolean is sent as-is so the API rejects it, instead of silently writing `false`.
- **`asset metadata get` prints `{}` (or CSV headers) for an asset without metadata** - stdout used to be empty for a successful command, and JSON output was wrapped in an internal `{"meta": ...}` object.
- **`folder download --folder-uuid` and `folder thumbnail --folder-uuid` no longer write into a directory named `untitled`** - The folder's name is used.
- **`folder upload` no longer copies every file through the temp directory** - The copy used a non-unique name, so two uploads of files called `part.stl` from different directories could clobber each other mid-upload.
- **`asset create --override` treats only "absent" as absent** - A network or session failure while checking for the existing asset used to be read as "does not exist".
- **A batch row whose folder does not exist is counted as missing, not as a permission problem.**
- **A configured API URL with a trailing slash no longer produces `//tenants` URLs; user IDs are URL-encoded.**
- **`tenant use` interactive selection no longer panics** on a tenant name containing a colon.
- **Excel report sorting parses each cell once** instead of once per comparison (tens of millions of parses on a million-row report).
- **`--tenant` accepts a tenant UUID, as its help text promised** - Only the short name worked. A tenant missing from the cached list is also looked up once more from the API before it is reported as not found, so a tenant granted since the cache was written is usable without `cache clear`.
- **`PCLI2_FORMAT` is honoured by `env list`, `env get` and `config get`** - `--format` has a default value, so the branch that consulted the variable was unreachable.
- **In-app advice no longer refers to a `context` command that does not exist** - "Run 'pcli2 context set tenant'" is now "Run 'pcli2 tenant use'".
- **Uploads go to the folder that was asked for** - The upload endpoint places a file by the path string it is sent and creates any folder it does not know, and three callers sent it the wrong string: `folder upload --folder-uuid` sent a placeholder `/` (every file landed at the tenant root), `asset create --folder-uuid` fell back to the bare folder name when the hierarchy fetch failed (a nested folder `A/B/C` became a new top-level `C`), and `--folder-path` was sent as typed, so `/Home/Parts` could create a literal `Home` folder and `/parts` a second folder beside `Parts`. The destination is now the folder's canonical path from the hierarchy, whichever way the folder was named.
- **Uploading to the tenant root works** - `asset create --folder-path /`, `asset create-batch --folder-path /` and `folder upload --folder-path /` failed with "Folder '/' not found" after fetching the whole hierarchy to suggest alternatives. The root has no UUID and needs none.
- **Typed metadata compares equal to itself in match reports** - A numeric or boolean field was rendered as text on the reference side and blanked on the candidate side, so the Excel diff painted every such field as "missing on one side" and CSV rows showed `42` against an empty cell. Both sides now render values the same way (JSON text for non-strings, empty for null).
- **Folder match JSON no longer invents reference asset fields** - `referenceAsset` reported `state: "active"` (not a Physna state), `type: "asset"`, `isAssembly: false` and empty timestamps regardless of the real asset. The real values are used.
- **Single-asset text match CSV no longer doubles the asset URL** - The stored URL was already an asset URL, and the CSV writer appended `/asset/<uuid>` to it again.
- **Excel reports stop at the worksheet hyperlink limit** - Excel allows 65,530 hyperlinks per sheet; a report between that and the 1,048,574-row cap produced a workbook Excel "repairs" by stripping every link. Comparison URLs past the limit are written as plain text, with one warning.
- **Comma-separated values work where the help text promised them** - `--name a,b` on `asset metadata delete` and `asset metadata inference`, and `--folder-path a,b` on `folder geometric-match`, `part-match`, `visual-match` and `asset dependencies`, were sent as one value containing a comma; a delete of `Material,Weight` deleted nothing and exited 0.
- **Metadata batch CSV: the classic header is checked and short rows are reported** - A file with columns in another order was applied positionally (paths and names swapped); `metadata get --format csv` output (`NAME,VALUE`) was accepted and silently did nothing because every row was too short. The header must now read `ASSET_PATH,NAME,VALUE[,TYPE]`; rows with fewer than three columns are counted and reported with line numbers; an empty NAME is an error.
- **Downloaded names cannot escape the output directory** - A folder or asset named `..`, or a name containing a path separator, is refused instead of written outside the directory the user chose.
- **One missing field no longer fails a whole page of assets** - `type`, `createdAt`, `updatedAt`, `state`, `isAssembly` and `metadata` are optional when deserializing, as are the page index fields.
- **Ambiguous folder paths are reported** - Two sibling folders that differ only by case (or not at all) both match a path; the first was used silently. A warning now names both UUIDs.

- **Batch commands exit non-zero when any item failed** - `asset create-batch` logged failed files at debug level only and printed the successes with exit 0; `asset metadata create-batch --continue-on-error` exited 0 with every row failed; `folder upload`, `folder download` and `folder thumbnail` printed "Operation completed successfully!" whatever had happened, and a missing thumbnail counted as a success. Every failure is now named on stderr and the command exits 69 (temporary failure) with a one-line count. `--continue-on-error` still decides whether the rest of the batch is attempted; it no longer decides whether the exit code tells the truth.
- **`folder download` without `--continue-on-error` stops at the first failure** - It used to attempt every remaining asset anyway (each with its own 5-10 second retry sleep) and only then report an error; an expired session on file 1 of 5,000 cost 5,000 failing attempts. Remaining downloads are now cancelled and counted as "not attempted".
- **Concurrent commands share one API client** - `folder download`, `folder thumbnail`, `folder upload` and `asset create-batch` built a fresh client inside every task: one credential-file read, one connection pool and one private token slot per asset, so a token expiring mid-run was renewed once per remaining asset (up to 22,000 auth-server calls in the largest known run) instead of once. Tasks now clone the caller's client, which shares the token, the renewal lock and the connection pool.
- **`folder upload` lists the destination folder once instead of once per file** - Each task listed the entire folder (every page) to check whether its file already existed: 2,000 files into a folder of 10,000 assets was about 100,000 requests before the first upload. A failed listing was also treated as "does not exist", so `--skip-existing` could re-upload on a transient error; it is now a hard error.
- **`folder download` and `folder thumbnail` no longer fetch every subfolder a second time** - The subfolder listing already carries the name the walk needed; one request per subfolder is gone.
- **Network and session failures while loading the folder hierarchy are no longer reported as "Folder not found"** - The lookup swallowed them and every caller turned the empty answer into a path problem, sending users to check a path that was right. They now surface as what they are, and `--recursive` no longer suggests `cache clear` for a failure that clearing the cache cannot fix.
- **`asset list` and `folder list` use the folder cache** - Both rebuilt the tenant's whole hierarchy from the API on every call (`asset list --recursive` twice), and `folder list --reload` refreshed nothing because the listing bypassed the cache. A path missing from the cached hierarchy triggers one refresh before it is reported absent, so a folder created since the cache was written is still found.
- **Downloads are streamed to disk through a temporary file** - `asset download` and `folder download` held the whole file in memory (ten concurrent multi-gigabyte assemblies meant ten files in RAM), wrote directly to the final name so an interrupted transfer left a truncated file that looked complete, and accepted an empty body as success. The file is now streamed into `<name>.part` and renamed into place only when whole; an empty response is an error. The blind "sleep and retry any error, including 404" wrappers are gone: the client retries transient failures itself.
- **Folder match runs count every failed task** - A task that failed outside the search itself was dropped from the tally, so "Searched X of Y" and the 10% threshold used a smaller denominator. A run stopped by repeated authentication failures now always exits non-zero, even when the stop happened past the 90% mark.
- **Match runs read the configuration once** - The UI base URL was loaded and parsed from disk for every match row: 1.29 million times on the largest known report.
- **"No assets to download" is a warning, not an error with exit 0** - `asset download-folder` and `folder download` printed a red error and exited 0 for an empty folder.
- **A server outage mid-run no longer produces a "complete" report with exit 0** - Every HTTP status other than 401/403/404 was wrapped in the same `Conflict` error the search endpoint uses for an asset that is not indexed, so a 500, 503 or 429 during a folder match was counted as "not searchable" - a property of the tenant, excluded from the failure threshold - and the run finished with a short report and a clean exit. Errors now carry their status: only a real 409 is "not searchable", and everything else counts as a failure that can fail the run.
- **Transient failures are retried on every request, not only on DELETE** - The API client built its requests on the raw HTTP client and skipped the retry helper entirely, so `PCLI2_MAX_RETRIES`, the backoff, and `Retry-After` applied to nothing the user cared about; one 503 in a 5,000-page match run aborted it. Every request now goes through one path with proactive token renewal, transient retry (connection errors, 408/429/502/503/504, `Retry-After` in seconds or HTTP-date form), and a single renew-and-retry on 401/403. Uploads re-open the file for each attempt; timed-out non-idempotent requests are still never resent.
- **Exit codes follow the documented contract** - Authentication failures exit 100, network failures 101, API errors 102, not-found 67, missing input files 66, an incomplete report 69, and usage errors from the argument parser 64 (previously 2). Every one of these used to exit 70 "internal software error" (or 0, see below), which made the `case $?` examples in the README impossible to hit.
- **Failures that exited 0 now exit non-zero** - `auth get` and `auth expiration` with no token, `env use` with no selection, `env get` with no active or an unknown environment, `tenant use` with no or an unknown tenant, `config validate` when validation fails, and `config import`, which was an unimplemented stub that reported success having done nothing and now says so.
- **Error messages are no longer rewritten by substring** - A folder named `401-series` was reported as an authentication failure, an unknown environment name was replaced by "Resource not found", a file called `connection-bracket.stl` became a network error. Messages are now printed as the code produced them, with one hint chosen from what the error actually is (a login hint for authentication failures, a role hint for a 403 that survived token renewal, a connectivity hint for network errors). The OAuth error codes from the auth server keep their explanations, appended rather than substituted.
- **Errors are printed once** - Login and token failures were reported with remediation steps and then printed a second time on exit as a bare "Security error"; the incomplete-report error was printed twice with different counts.
- **A 404 no longer triggers a token renewal** - Every not-found answer (a deleted asset, a wrong UUID) cost an auth-server call, a credential-file write and a retried request, and with a token-only login was then reported as an expired token. A 404 is now a not-found.
- **Automatic re-authentication failures say why** - A rotated secret, a rate-limited auth endpoint and a DNS failure all printed the same "log in again"; the cause is now included.
- **Concurrent 401s collapse into one renewal** - The check for "someone else already renewed" compared against the token current at the moment of the check rather than the token the failed request had used, so a task that read the shared slot after another task's renewal renewed again anyway.
- **Timeouts** - A stalled connection used to wait out the full 30-minute request timeout even for a small JSON call, and the auth client had no timeout at all while holding the renewal lock, which hung every concurrent task with it. Connections now time out after 15 seconds and a read after 5 minutes of silence; the total timeout (`PCLI2_TIMEOUT`, still 30 minutes by default) only bounds genuine long transfers.
- **Every request identifies itself** - The `User-Agent` header (now `PCLI2/<version>`) was only sent on DELETE.
- **Unsupported file types are no longer retried five times by `--override`** - The "Invalid path extension" upload rejection was classified as a conflict, which the override path treats as "the previous asset may still be deleting" and retries with backoff. It is now an invalid-parameter error, reported the same way whether or not the first attempt also hit an expired token.
- **Thumbnail downloads on a Viewer account no longer make three auth-server calls per asset** - The per-asset retry helper renewed the token up to three more times on any 403; the client's own renew-and-retry is enough.

### Documentation
- The install guide had pointed at the wrong repository and listed a Rust toolchain as a prerequisite for the binary install; it now covers the shell and PowerShell installers, the Homebrew tap, the MSI, `pcli2-update`, and says plainly that the crate is not on crates.io. The README no longer claims credentials are kept in the OS keychain: they live in `dev_credentials.json` under the configuration directory, owner-only on macOS and Linux. `pcli2 config environment ...` became `pcli2 env ...` some releases ago and the README and quick start now say so; `asset geometric-match-folder` is `folder geometric-match`; the `context` command is `tenant use/get/clear`; `--path` on `asset list` and `folder download` is `--folder-path`; examples use `/Home/...`, the name Physna shows for the root, instead of `/Root/...`. Three environment variables (`PCLI2_API_BASE_URL`, `PCLI2_UI_BASE_URL`, `PCLI2_AUTH_BASE_URL`) that the code never read are gone from the docs, and every variable that is read is now listed in one table (README and Cross-Platform Configuration). The exit-code tables drop the code that is never emitted (68) and explain 64 for parser errors and 69 for partial batches. The documented multi-level inference propagation and the `config import` example are removed with the features that did not exist.

### Removed
- **Dead dependencies** - `bincode`, `exitcode`, `url`, `mime`, `tokio-util` and `async-recursion` (recursion now uses `Box::pin`, stable since Rust 1.77); `tempfile` moves to the test dependencies and `tokio-test` is dropped. Two integration test files that compiled to no tests (one was a single line of escaped text; the other asserted nothing) are removed and the test README describes what actually exists. The committed mdBook output under `docs/book/` and the stale top-level `docs/*.md` copies are removed; the published site builds from `docs/src/`. The site no longer advertises `cargo install pcli2`.

### Changed
- **CI runs `cargo audit` and a minimum-supported-Rust check (1.88, set by transitive dependencies)**; `rust-version` is declared in `Cargo.toml`. `Cargo.lock` is no longer listed in `.gitignore` (it was tracked all along).

- **`asset metadata inference --recursive`** - The flag was accepted and never read; the documented multi-level propagation did not exist.

### Changed
- **Batch status lines go to stderr** - The statistics reports of `folder upload`, `folder download` and `folder thumbnail`, and the per-file "Successfully uploaded" / "Skipping existing asset" lines, no longer share stdout with command output.

## [1.20.0] - 2026-09-04

### Fixed
- **`asset create --override` no longer deletes the existing asset before checking the local file** - The existing asset was deleted first and the local file was only validated by the upload that followed, so a mistyped filename (or an unsupported extension, a 413, a dropped connection) destroyed the customer's asset and its metadata with no way back. The local file is now checked before anything destructive happens, and if the re-upload still fails after the delete, the deleted asset's UUID and any metadata captured for `--restore-metadata` are printed so they can be recovered by hand.
- **`folder list --folder-uuid` now lists that folder** - The argument was accepted by the parser and never read, so the command silently listed the tenant root instead.
- **The banner no longer corrupts command output** - It was printed to stdout whenever *any* argument equalled `help`, `-h` or `--help`, including values: `env list --name help` or `asset text-match --text help` emitted ASCII art ahead of their JSON. The banner is now shown only above genuine help output, as decided by the argument parser.
- **`--threshold` is validated as a percentage** - Values outside 0-100 are rejected at parse time on every matching command (`--threshold 500` used to go straight to the API; `--threshold -5` failed with a misleading "unexpected argument"). A value below 1, almost always `0.85` typed for 85%, is still accepted but produces a warning saying what it actually asks for.
- **`--concurrent` is validated on every command, so `--concurrent 0` no longer hangs** - `asset create-batch` sized its semaphore from the flag with no range check, and zero permits made every upload wait forever with no output. The folder match commands only rejected an out-of-range value after resolving the tenant. All concurrent commands now share one parser that accepts 1-10.
- **The install page links to the right repository** - `docs/src/installation.md` pointed the installer script, the release page, the clone URL and the issues link at `github.com/physna/pcli2`, which is not this project.

### Changed
- **CI is green again on the current stable toolchain** - clippy 1.98 flags every `Result<_, CliError>` as `result_large_err` because the CSV writer error embedded in the error enums was several hundred bytes wide. That variant is now boxed in both `FormattingError` and `CliActionError`; nothing user-visible changes.
- **Dependency lockfile refreshed** - Clears the `h2` advisory (RUSTSEC-2026-0258, reachable through reqwest in the shipped binary) and the `rand` and `event-listener` unsoundness advisories reported by `cargo audit`.

## [1.19.0] - 2026-08-01

### Changed
- **The new-version hint now also follows a failed command, `--version`, and a rejected argument** - It previously appeared only after a *successful* run, which is when it matters least. The motivating case is the opposite one: a user on a build predating the flag they are passing sees `unexpected argument`, concludes the feature is broken, and has nothing pointing at their own install. That is not hypothetical — it is the `--recursive` report described in the note below, which cost a support round trip and a release spent chasing a defect that user never hit. Argument parsing no longer lets clap exit on pcli2's behalf, so a usage error or a `--version` check can still be told the binary is out of date, and a failed command prints the hint after the error so it is the last thing read. `--help` is excluded: its output is long enough that a trailing line would scroll past unread. Every existing restraint still applies — terminal sessions only, at most one check per 24 hours, never in CI, and `PCLI2_NO_UPDATE_CHECK` still disables it entirely.

### Notes
- **Correction to the v1.18.3 entry below.** That fix was attributed to a field report of `--recursive` "not working". The report turned out to have a different cause — an old `pcli2` earlier on the user's `PATH`, in a shell where `--recursive` did not yet exist — so the silent-fallback defect it describes was *not* what that user hit. The defect is real and the fix stands: a recursive scan really could degrade to listing only a folder's direct children, with the explanatory warning painted over by the `--progress` spinner. But it was found by reasoning backwards from the symptom rather than by reproducing it, and there is no evidence it has ever triggered in the field. Worth knowing before treating a low recursive count as that bug: check `pcli2 --version` in the shell actually being used first.

## [1.18.3] - 2026-07-31

### Fixed
- **`--recursive` no longer degrades silently into a non-recursive scan** - When the folder hierarchy could not be loaded, a recursive folder match fell back to listing only the assets sitting directly in the folder. The warning that was supposed to explain it went to stderr underneath the `--progress` spinner, which redraws every 100ms and painted straight over it — so the run looked like a success and simply reported far too few assets. Reported from the field as "--recursive is not working": a folder holding 22,378 assets across 511 subfolders reported **1**, the count of assets directly inside it. A recursive scan that cannot enumerate subfolders now fails with an explanation and a suggestion to run `pcli2 cache clear`, rather than quietly answering a different question. The same applies when the folder resolves against the API but is missing from the cached hierarchy, which previously produced a silent empty result. Scan errors also clear the spinner before printing, so the message survives.
- **`--continue-on-error` now takes precedence over the permission stop** - A `403` that survives a token renewal stops a metadata batch, since without per-asset permissions every remaining write would fail the same way. `--continue-on-error` means what it says, so it now wins: the account-level explanation is given once and the run continues, with the usual terse per-asset skip lines.

## [1.18.2] - 2026-07-31

### Fixed
- **A batch that fails because the account lacks the Author role now says so** - Physna has no per-asset permissions: an account is an Author, who may write any asset, or a Viewer, who may write none. A Viewer running `asset metadata batch` therefore fails on every write, but the run reported an authentication failure and told the user to re-authenticate — a session that was working perfectly. `pcli2` now distinguishes *authenticated but not permitted* from *credentials not working*: a `403` that survives a token renewal is reported as a role problem, with guidance to ask a tenant administrator for the Author role, and stops the run immediately rather than repeating the same failure for every remaining asset. A `401` that survives renewal still means the credentials themselves are the problem and still asks the user to log in again.

## [1.18.1] - 2026-07-31

### Fixed
- **A transient authentication blip no longer aborts an entire metadata batch** ([#93](https://github.com/jchultarsky101/pcli2/issues/93)) - `asset metadata batch` stopped the whole run and told the user to re-authenticate a session that was working, so following the advice changed nothing and the next attempt failed identically. A 401/403 causes the client to renew the token and retry; the resulting error text always embeds the original 401/403, and the authentication check searched the whole message — so it matched *every* retried failure unconditionally, whatever the retry actually returned. A stale-token 403 whose retry then failed for an unrelated reason (the asset not being indexed, a metadata type conflict, a 5xx) was reported as an authentication problem. The check now inspects the retry's own status rather than the original or the response body, and the decision to abandon a run uses a narrower test that fires only when the credential renewal itself failed. Everything else is treated as a per-asset failure: counted, reported with relevant guidance, and skipped under `--continue-on-error`. This also affects the concurrent folder match commands, where the same check feeds the consecutive-failure counter — three assets in a row failing this way would have tripped a spurious stop. Asset lookups that fail for a reason other than the asset being absent are also no longer reported as "asset not found".

## [1.18.0] - 2026-07-31

### Fixed
- **Token renewal is now shared between concurrent tasks** ([#86](https://github.com/jchultarsky101/pcli2/issues/86)) - The API client is cloned once per task by the concurrent commands, and the access token was a plain field, so a token renewed inside one task was invisible to every other task — each of which still held the original and renewed again for itself. On a folder tree of ~22,400 assets, one expiry produced up to ~22,000 separate token requests where one would do, each an opportunity to fail. The token is now shared by a client and all of its clones, and concurrent renewals are collapsed: the first task to notice renews, and the rest pick up its result instead of queueing to repeat it. Tokens are also renewed shortly before they expire rather than after a request has already been rejected, which avoids the failed-request-then-retry round trip entirely; that check is best-effort and can only avoid work, never fail a request that would otherwise have succeeded.

## [1.17.0] - 2026-07-31

### Fixed
- **Folder match output is now reproducible** ([#82](https://github.com/jchultarsky101/pcli2/issues/82)) - Two identical runs over unchanged data produced reports that differed byte-for-byte. Rows were collected in whatever order the concurrent searches finished, and for a symmetric pair the surviving row was whichever direction arrived first — so which asset appeared as the *reference* varied between runs. Matches are now deduplicated into an ordered map keyed on the unordered pair, so both the row order and the choice of orientation are properties of the data rather than of the run's scheduling. When a pair is searched from both ends the record whose reference sorts first is kept; a pair searched from only one end keeps that direction, and no record is ever rewritten to look canonical, since the transformation matrix and comparison URL describe one direction and flipping them would corrupt both. Two runs over unchanged data now produce identical bytes, so reports can be diffed and version-controlled.
- **A folder match run now stops after repeated authentication failures, instead of issuing thousands of doomed requests** ([#78](https://github.com/jchultarsky101/pcli2/issues/78)) - When the token expired partway through a long run and automatic re-authentication failed, every remaining asset failed identically: 21,068 of them on one run, each logging its own copy of the same error, taking the full ~25 minutes and ~21,000 pointless API calls to finish. A run now stops after three *consecutive* authentication failures, and the reason is reported once rather than once per asset. Consecutive, not the first: the client renews the token and retries automatically — which is what lets a long run survive its credentials expiring halfway through — and a failed renewal is reported the same way whether the credential was rejected or the auth endpoint merely hiccuped, so stopping on the first would abandon a long run over something the next request would have recovered from. Any successful search resets the count, so scattered blips are absorbed while a genuinely dead credential trips it almost immediately. Assets that were never searched are counted separately from ones that were tried and failed (`Searched 1,259 of 22,378 asset(s): 1 failed, 21,067 not attempted, 51 not searchable`).
- **Folder match commands no longer report success when most of their searches failed** ([#77](https://github.com/jchultarsky101/pcli2/issues/77)) - A failed per-asset search contributed nothing and was otherwise indistinguishable from an asset that genuinely had no matches, so nothing counted failures and nothing could report them. A run that lost 21,119 of 22,378 searches to an expired token still exited `0` and printed a normal completion summary, handing over a report missing 94% of its rows. Failures are now counted and reported whenever any occur (`Searched 1,259 of 22,378 asset(s): 21,068 failed, 51 not searchable`), and the command exits non-zero when operational failures exceed 10% of the assets. Failures are classified: an asset that cannot be searched in its current state (not indexed, no 3D data, indexing failed - reported by the API as a `409`) is a property of the tenant and never fails the run, while authentication, network, and 5xx failures do. The check runs immediately after matching, so an incomplete run stops before spending minutes building a report that should not be trusted. Applies to `folder geometric-match`, `folder part-match`, and `folder visual-match`.

## [1.16.0] - 2026-07-30

### Changed
- **`folder geometric-match --format xls` now refuses a report too tall for a worksheet, instead of failing deep inside the workbook writer** - An Excel worksheet holds 1,048,576 rows; this report spends two of them on its header band, leaving 1,048,574 for data. A real run produced 1,293,068 rows, which cannot be represented. The command now fails immediately after matching with `the report has 1293068 rows, more than the 1048574 an Excel worksheet can hold - use '--format csv' for the complete report`, before building a single row. Truncating to fit was considered and rejected: a workbook silently missing a quarter of its rows looks complete to whoever opens it. Use `--format csv`, which has no such limit.

### Security
- **The stored-credentials file is now owner-only (`0600`)** - With the default `dev-keyring` feature, the client secret and access token are kept in plain text in `dev_credentials.json`. Written under the process umask it landed at `0644`, readable by every other account and every unprivileged process on the machine. The mode is now set explicitly on every write, and an existing file is tightened the first time it is read, so credentials written by an earlier version are repaired rather than left exposed until the next login. Unix only; Windows has no equivalent mode bits and inherits directory ACLs.

### Fixed
- **`auth login` now prompts when the stored credentials are empty, instead of failing with `invalid_client`** - An empty stored client ID or secret was treated as a usable credential and sent to the auth server, which rejected it with an error pointing at the user's credentials when the actual problem was local state. Empty is now treated as absent, falling through to the interactive prompt — matching what the prompt path itself already did.
- **Report building no longer materializes an intermediate copy of every match** - `folder geometric-match` built a `Vec<GeometricMatchPair>` before building rows, cloning each reference asset — metadata `HashMap` included — once per row, only ever to read a path and a UUID back out of it. On a 1.29M-row report that intermediate took seconds to build and a further nine seconds to *drop*, with the progress display parked at 99% for the whole teardown. Both passes now iterate borrowed data. Output is unchanged.
- **The progress bar no longer flashes `0/0 (0%)` when a counted phase starts** - The steady-tick thread could redraw in the window between the counter style being applied and the row count being set. Length is now set first.

## [1.15.0] - 2026-07-30

### Changed
- **JSON match reports now stream to `stdout` too** - Completes what v1.14.0 started for CSV. `folder geometric-match`, `folder part-match`, and `folder visual-match` built the entire pretty-printed JSON document as one `String` before printing it; on a large report that is a second full copy of the data — pretty-printed JSON is bulkier than the rows it came from — and nothing reached the terminal until the last byte was ready. Serialization now goes straight to a buffered, locked `stdout`. The bytes written are unchanged, trailing newline included. This supersedes the note in the v1.14.0 entry below that JSON would keep buffering: the atomicity trade is real and is harsher here than for CSV, because a truncated JSON document does not parse at all where a truncated CSV still does. A mid-write failure still reports on stderr with a non-zero exit code, which is what separates it from a silently short file. The single-asset `asset match visual` and `asset match text` reports are small and still buffer.

## [1.14.0] - 2026-07-30

### Changed
- **CSV match reports now stream to `stdout` instead of being buffered whole** - `folder geometric-match`, `folder part-match`, and `folder visual-match` used to serialize the entire report into a byte buffer, copy that into a `String`, and print it in one go, holding the report in memory three times over before emitting a single byte. Rows now go straight to a buffered, locked `stdout` as they are produced: output starts appearing immediately, two of the three full copies are gone, and peak memory no longer scales with a second and third copy of the report. The bytes written are unchanged. Two consequences worth knowing: a failure partway through now leaves a truncated report on `stdout` rather than printing nothing at all (the error still goes to stderr and the exit code is still non-zero), and closing the pipe early — `pcli2 … | head` — now ends with a clean error instead of a `failed printing to stdout` panic. JSON output deliberately still buffers, because half a JSON document is not a document.

### Added
- **`--progress` now covers report building as well as matching** - On a large result set, folder match reports spend minutes turning matches into a table and serializing them, and that work happens *after* the match progress bar has already reached 100% — so the command looked wedged (one run sat silent for several minutes at 22,378 assets). With `--progress`, each phase now reports itself with a running elapsed time, and the phases whose size is known up front (flattening matches, collecting metadata columns, building rows, formatting CSV, writing worksheet cells) show a percentage bar and an ETA rather than an indeterminate spinner. Serializing JSON, sorting the workbook, sizing its columns, and saving it are named as they happen, so a long wait can be attributed to a specific step. A summary line closes the report (`Built report of 1,743,201 row(s) in 2 minutes`). Applies to `folder geometric-match`, `folder part-match`, and `folder visual-match`, in every output format including `--format xls`. All of it goes to stderr, so piped `stdout` is unaffected; without `--progress` the phase stays silent.
- **`--progress` now covers the recursive folder scan** - With `--recursive`, folder match reports spend time walking the folder tree (one API call per folder) before any matching starts, which looked like a hang on a deep tree. When `--progress` is passed, that phase now reports live (`Scanning /Creo Files: 46/312 folders, 1174 assets found`) and prints a summary before matching begins (`Scanned 1 folder path(s), found 3182 asset(s) to match`). All of it goes to stderr, so piped `stdout` is unaffected; without `--progress` the scan stays silent.

## [1.13.0] - 2026-07-30

### Added
- **`--recursive` for folder match reports** - `folder geometric-match`, `folder part-match`, and `folder visual-match` accept `--recursive` (`-R`) to include the assets in every subfolder, not just those sitting directly in the named folder. The default is unchanged, so existing commands keep their current scope and runtime; recursion is opt-in because it can widen the work dramatically (a folder with a single asset of its own may have thousands underneath it, and each one costs a search).

### Fixed
- **Folder match reports no longer look like a broken path on container folders** - A folder that holds nothing but subfolders has no assets of its own, so a match report over it found nothing and failed with `No assets found in the specified folder(s)`, whose first suggestion was to verify the folder path. The path had in fact resolved correctly, which sent users hunting for a typo that was never there. The message now reports how many subfolders the folder actually contains and points at `--recursive`:

  ```
  ❌ Error: No assets found directly in the specified folder(s)

  🔧 To resolve this issue, try the following:
    1. The folder(s) contain 8 subfolder(s) - pass --recursive to include the assets in them
  ```

## [1.12.0] - 2026-07-20

### Changed
- **`asset text-match` no longer silently drops results** - Text search ranks name matches above metadata-only matches, so the previous default `--limit` of 100 could silently omit assets whose only match is in a metadata value. The default limit is now 1000 (the limit remains as a guard against excessive queries), and when the limit does cut the result set short a warning is printed to stderr (`⚠️  Result limit reached: showing the first N matching assets; increase --limit to retrieve more`). The warning goes to stderr only, so JSON/CSV data piped from stdout is unaffected.

## [1.11.0] - 2026-07-13

### Added
- **`tenant metadata list` command** - Lists every metadata field registered in the tenant along with its data type (`text`, `number`, `boolean`, `url`). Supports `--format json|csv|tree` (alias `ls`). The CSV output uses the same column header as the classic `asset metadata create-batch` input (`ASSET_PATH,NAME,VALUE,TYPE`), with `NAME` and `TYPE` filled from the registry and `ASSET_PATH`/`VALUE` left blank — so a listing can be saved and turned directly into a batch-upload template.
- **Optional `TYPE` column for `asset metadata create-batch`** - The classic CSV layout accepts an optional fourth `TYPE` column (`text`, `number`, `boolean`, or `url`; default `text`), applied per row. It sets the type used when *registering a new* metadata field; for a field that already exists, its registered type stays authoritative. The `url` type is now also accepted by single `asset metadata create --type`.

### Fixed
- **Batch metadata upload no longer fails on non-text fields** - `asset metadata create-batch` sent every CSV value as a string, so `number`, `boolean`, and `url` fields were rejected with a "type mismatch" error. Values are now automatically coerced to each field's registered type (e.g. `18` for a number field, `true` for a boolean, a URL string for a url field), so existing typed fields populate without any extra configuration. A value that cannot be represented as the field's type (e.g. `N/A` for a number field) is reported as a type conflict.
- **`/Home` asset paths in batch uploads resolve to the root** - A leading `/Home` (the name Physna shows for the root folder) in an `ASSET_PATH` is normalized to the root, so `/Home/NX/part.prt` and `NX/part.prt` refer to the same asset. Rows that spell the same asset differently now merge onto one asset instead of splitting into separate lookups.

### Changed
- **`asset metadata create-batch --continue-on-error` also skips metadata failures** - Previously the flag skipped only assets whose path could not be resolved; a failed metadata update or delete (such as a type conflict) always terminated the batch. It now skips the offending asset and continues, consistent with how unresolved paths are handled. Authentication failures remain fatal.

## [1.10.0] - 2026-07-02

### Added
- **Homebrew formula published automatically** - Releases now push an updated formula to the `jchultarsky101/homebrew-pcli2` tap via cargo-dist, so `brew install jchultarsky101/pcli2/pcli2` installs the current version (the tap had been stuck at v1.0.0). Requires the `HOMEBREW_TAP_TOKEN` repository secret.
- **`PCLI2_TIMEOUT` environment variable** - Overrides the request timeout (seconds). The default remains 30 minutes, which large model transfers require; users working with small files can opt into faster failures.

### Fixed
- **Redirected diagnostics are free of ANSI escape codes** - tracing output (warnings, debug logs) emitted color codes even when stderr was redirected to a file. Colors on stderr now follow the same rules as stdout: disabled when not a terminal, or when `NO_COLOR`/`PCLI2_NO_COLOR`/`--no-color` is set.

### Removed
- **`update-latest-tag.yml` workflow** - It triggered on `release: published`, an event GitHub suppresses for releases created with `GITHUB_TOKEN`, so it had not run since October 2025. Nothing consumes the `latest` git tag it maintained (installers use GitHub's `releases/latest` URLs, which are independent); the stale tag has been deleted.

### Changed
- **Timed-out requests are retried only for reads** - A network timeout can fire after the server has started processing a request, so retrying a timed-out write (POST/PUT/DELETE) could apply an operation twice. Timeouts now retry only GET requests; connection failures (request never reached the server) and transient status codes (408/429/502/503/504) retry for all methods as before.
- **Failed update checks are not re-attempted until the next daily window** - Previously an unreachable GitHub API (offline, firewalled, rate-limited) caused the version check to be re-attempted on every command once its cache went stale, adding up to 3 seconds per command. A failed check now counts as the day's attempt.
- **Man page SYNOPSIS shows the real invocation** - Pages now read `pcli2 folder delete [OPTIONS]` instead of `pcli2-folder-delete [OPTIONS]`, while page names/headers keep the git-style dashed form (`pcli2-folder-delete(1)`).

## [1.9.0] - 2026-07-02

### Added
- **`--dry-run` flag for destructive and bulk commands** - `asset delete`, `folder delete`, `asset create`, `asset create-batch`, and `folder upload` can now report exactly what they would delete or upload and exit without making any changes. For `folder upload` the check runs before remote folder resolution, so a dry run never creates the target folder.
- **Global `--verbose`/`-v` and `--quiet` flags** - Quick verbosity control on every command: `--verbose` enables debug-level logging, `--quiet` limits diagnostics to errors. Both take precedence over the `PCLI2_LOG_LEVEL`/`RUST_LOG` environment variables. The local `--verbose` on `config validate` is replaced by the global flag (same behavior).
- **Automatic retries for transient failures** - Network timeouts, connection errors, and HTTP 408/429/502/503/504 responses are now retried with exponential backoff and jitter, honoring the server's `Retry-After` header. Default is 2 retries; `PCLI2_MAX_RETRIES` overrides (0 disables).
- **`pcli2 man` command** - Generates Unix man pages for pcli2 and every subcommand (one page per command, e.g. `pcli2-folder-delete.1`) into a directory given by `--output-dir`.
- **Update notifications** - After a successful command in an interactive terminal session, pcli2 prints a one-line stderr hint when a newer release is available on GitHub. Checked at most once per 24 hours with a 3-second timeout; skipped in CI, for redirected output, and for `completions`/`man`; opt out with `PCLI2_NO_UPDATE_CHECK`.
- **Spinners for quick operations** - Single API round-trips (`asset get`, `tenant list`, `config validate --api`) show a spinner on stderr so the CLI never appears hung. Hidden automatically when stderr is not a terminal.
- **Scripting and Automation documentation** - New docs page (and README sections) covering exit codes, machine-friendly output, dry-run mode, retries, and a GitHub Actions integration example.

### Changed
- **Interactive `auth login`** - When `--client-id`/`--client-secret` are omitted and no stored credentials exist, pcli2 now prompts for them interactively (masked input for the secret, keeping it out of shell history) instead of erroring. Non-interactive sessions keep the previous missing-argument error.
- **Colors are TTY-aware and respect `NO_COLOR`** - The banner, help examples, and help styling no longer emit ANSI escape codes when output is piped or redirected, when `NO_COLOR`/`PCLI2_NO_COLOR` is set, or when the global `--no-color` flag (previously defined but inoperative) is passed.
- **Consistent progress bars** - All overall progress bars now show ETA and throughput (the batch upload, asset download, and batch create bars were missing one or both), and per-file spinners show elapsed time.

### Fixed
- **Warnings are no longer printed twice** - Warnings (e.g. skipped rows in `asset metadata create-batch --continue-on-error`) were emitted both through tracing and a direct stderr print. They now go through tracing only, so their visibility is controlled by `--verbose`/`--quiet`, `RUST_LOG`, or `PCLI2_LOG_LEVEL`. The tracing subscriber also writes to stderr (previously stdout), so diagnostics never pollute piped command output.
  - **Note for script authors**: warning lines now use the tracing format (`<timestamp> WARN <module>: <message>`) instead of the previous `⚠️  Warning: <message>` prefix - scripts that matched the old literal prefix on stderr need updating.
  - **Note on `--quiet`**: warnings are suppressed under `--quiet` (errors only). End-of-run summaries (batch statistics and remediation blocks) are still printed.

## [1.8.2] - 2026-07-02

### Fixed
- **Nonexistent folder paths no longer silently resolve to the root folder** - A mistyped `--path`/`--folder-path` (e.g. `asset delete --path "/Typo/part.stl"`) previously fell through to the ROOT folder and, because assets are matched by name within the resolved folder, could target a same-named asset at the root. A non-root path that doesn't resolve is now a "Folder not found" error. An absent or `/` folder path still means the root, as before.
- **Folder cache is invalidated on folder create/delete/rename/move** - Mutating folder operations previously left the cached folder hierarchy stale for up to an hour, so path resolutions could target deleted/renamed folders (deleting the same folder twice even reported the misleading "Folder is not empty" error).
- **Metadata values are no longer rewritten on upload** - `asset metadata create` and `create-batch` silently substituted characters in text values (`Ø`→`O`, `°`→`" deg"`, `″`→`"`, `…`→`...`), corrupting engineering metadata. Values now round-trip verbatim.
- **Pagination fixes across the API client**:
  - Three endpoints sent the pagination page size as `per_page` instead of `perPage`, so the API served its default 20 items per page: asset dependencies (×2) and `assets/state` listing. Dependency traversal now makes ~5× fewer API calls, and the state listing's safety cap is effectively 200,000 assets instead of 20,000.
  - `asset match text` fetched only the first 50 results and presented them as complete. It now paginates through all matches and accepts `--limit` (default 100), consistent with `visual-match`.
  - `part-search` was silently capped at 5,000 matches (50 pages); the cap is now 1,000 pages, aligned with visual search, and hitting any pagination safety cap prints a visible truncation warning instead of a debug log.
  - `folder download`/`folder thumbnail` read only the first 1,000 direct subfolders of each folder; folders beyond that (and their entire subtrees) were silently skipped. All subfolder pages are now walked.
  - `user list` returned an empty list if the API responded without pagination data; accumulated users are now returned.
  - `geometric-search` gained the same stall guard as its siblings (protects against infinite pagination loops), and mid-pagination responses without page data no longer discard previously accumulated matches.
- **Auth and error handling**:
  - A 404 that persists after the automatic token refresh is now classified as a not-found error instead of "Conflict", and dependency queries no longer swallow unrelated auth errors into an empty dependency list.
  - `asset metadata create-batch` now detects authentication failures that persist through token refresh (previously each remaining CSV row triggered a redundant refresh+retry cycle instead of aborting).
  - The authentication retry of a file upload now sends the identical multipart form as the first attempt (it previously disabled `createMissingFolders`, so an upload interrupted by token expiry could fail where a fresh attempt would succeed).
  - Concurrent batch-upload clients now inherit the configured auth URL and environment; mid-batch token refreshes previously hit the production auth endpoint and saved tokens under the wrong environment.
  - Keyring read errors (e.g. locked keychain) are now logged instead of being silently treated as "not logged in".
- **`--exclusive` match filtering respects folder boundaries** - A `/proj` filter previously also matched sibling folders like `/proj-archive` (bare string-prefix comparison).
- **`folder download --resume` now skips already-downloaded assemblies** - Assemblies download as a ZIP that is extracted and deleted, so the resume check (which looked for the ZIP) re-downloaded every assembly on every resume. The extracted assembly file is now used as the marker.
- **`asset download-folder` no longer packages stale files** - The reusable temp directory is cleared at the start of each run, so leftovers from a previously failed run can't leak into the output ZIP.
- **Download/upload statistics are accurate** - Skipped files were counted as successes; the "Skipped (already existed)" count was always 0 for downloads and counted subdirectories for uploads.
- **Failed confirmations and failed cache clears exit non-zero** - `asset delete`/`folder delete` exited 0 without deleting when the confirmation prompt failed (e.g. non-TTY without `--yes`); `cache clear` printed "cleared successfully" even when every purge failed. Both now report errors. `cache clear` also clears the tenant cache for all environments and the actual metadata cache file.
- **Environment variable handling** - `PCLI2_FORMAT` set to a format not supported by a given command (e.g. `tree`) no longer aborts that command; an explicit `--format` always wins and unsupported env values fall back to the default. `PCLI2_HEADERS` now accepts `1`/`0`/`yes`/`no` in addition to `true`/`false`.
- **Crash fixes** - A batch CSV header containing a multi-byte UTF-8 character at the `metadata:` prefix boundary panicked the parser; `asset thumbnail --file thumb.png` (bare filename) was wrongly rejected with "Parent directory does not exist"; unfollowed HTTP 3xx responses could panic the HTTP layer.
- **`asset metadata inference` reporting** - The output now distinguishes `fields_requested` from `fields_copied` (previously all requested fields were reported as copied), and the reference asset no longer matches itself into the updated-assets count.

## [1.8.1] - 2026-07-02

### Fixed
- **Metadata field listing now retrieves all pages** - `get_metadata_fields` fetched the `/tenants/{id}/metadata-fields` endpoint without pagination parameters, so only the first page (20 fields) was returned. For tenants with more metadata fields this had two consequences: the type-mismatch pre-check during metadata updates silently did not protect fields beyond the first page, and every batch update issued a doomed field-registration attempt (HTTP 409 "already exists") for each field the truncated list was missing. The client now walks all pages (200 per page). Affects `asset metadata create-batch` and any command that registers or type-checks metadata fields.

## [1.8.0] - 2026-07-02

### Added
- **`--delete-if-empty` flag for `asset metadata create-batch`** - Gives precise control over what an empty value in the input CSV means, in both the classic and UI layouts (default: `false`).
  - **Without the flag**, empty values are skipped: the existing metadata field on the asset, if any, is left untouched, so a sparse file can be used to incrementally add or update fields. Skipped classic-format rows are reported with a single aggregate warning that points at the flag.
  - **With the flag**, an empty value deletes that metadata field from the asset - useful for replacing an asset's metadata wholesale. This also brings the UI format to parity with the classic format: empty `metadata:` cells (including rows whose cells are all empty) become deletions, whereas previously the UI format could not delete at all.
  - Affects `pcli2 asset metadata create-batch` (and its `update-batch` alias).

### Changed
- **Empty values in classic-format batch metadata CSVs no longer delete by default** - Previously an empty `VALUE` in the classic `ASSET_PATH,NAME,VALUE` layout always deleted the metadata field. Deletion is now opt-in via `--delete-if-empty`; without it, empty values are skipped with a warning.

## [1.7.0] - 2026-07-01

### Added
- **`--threshold` option for visual match commands** - `asset visual-match` and `folder visual-match` now accept `--threshold` (short `-s`, default `80.0`), consistent with `geometric-match` and `part-match`. For visual search the value maps to the API's `sizeThreshold`: it filters matches by geometric size relative to the reference asset (higher is stricter; `0` disables size filtering).

### Changed
- **Visual search migrated to the new Physna API endpoint** - `asset visual-match` and `folder visual-match` now call `POST /tenants/assets/visual-search` (operation `CrossTenantVisualSearch`) instead of the soon-to-be-deprecated `POST /tenants/{tenantId}/assets/{assetId}/visual-search`. All inputs, including pagination, are carried in the request body; the search remains within the current tenant (the same tenant is passed as both the asset owner and the search target). The response format and command output are unchanged.

## [1.6.0] - 2026-07-01

### Added
- **Physna UI CSV format support for `asset metadata create-batch`** - The command now accepts the horizontal CSV layout used by the Physna web UI's bulk metadata upload, in addition to the existing vertical `ASSET_PATH,NAME,VALUE` format. The layout is auto-detected from the header row (any column starting with `metadata:` selects the UI format) and can be forced with the new `--csv-format <auto|classic|ui>` option (default: `auto`).
  - The UI format has one row per asset: a `path` column, an optional `id` column holding the asset UUID, and one `metadata:<field name>` column per metadata field (the prefix is stripped to obtain the field name).
  - When a row's `id` is present and non-empty, the UUID **takes precedence** over the path and is used directly. An invalid UUID is an error - there is no fallback to the path, since that could silently target a different asset.
  - **Empty metadata cells are skipped** (existing values are left untouched), unlike the classic format where an empty VALUE deletes the field. Spreadsheet-style exports naturally contain blank cells, so blanks are never treated as deletions in this layout.
  - Columns other than `path`, `id`, and `metadata:*` are ignored with a warning. The whole file is parsed and validated (including UUID syntax) with line-numbered errors before authentication or any API call, so a malformed file fails fast instead of half-applying.
  - The classic vertical format behavior is unchanged; existing CSV files continue to work without any flag.
  - Affects `pcli2 asset metadata create-batch` (and its `update-batch` alias).

## [1.5.0] - 2026-07-01

### Added
- **`asset dependency-diff` command** - Compare the recursive dependency trees of two assemblies (a **reference** and a **candidate**) and report which parts differ between them. Each asset is identified by **either** a UUID **or** a path, consistent with the other asset commands: `--reference-uuid`/`--reference-path` and `--candidate-uuid`/`--candidate-path` (each pair is mutually exclusive and required).
  - The comparison is a **structural** tree diff: the two trees are walked in parallel and nodes are matched by **filename**. It is **presence-only** - each node is reported as present in both (`=`), only in the reference (`-`), or only in the candidate (`+`); occurrence counts are not compared. A subassembly present on only one side has its entire subtree marked accordingly.
  - Output supports `tree` (default view of the merged diff, with a legend and summary line), `json`, and `csv` (columns `STATUS`, `ASSEMBLY_PATH`, `FILENAME`, `ASSET_UUID`, `ASSET_STATE`), mirroring the existing `asset dependencies` command.
  - If either asset cannot be resolved, the error identifies which input (reference or candidate) failed. An asset that is not an assembly is treated as having no dependencies.
  - Available under the alias `asset deps-diff`.
  - Affects `pcli2 asset dependency-diff` (and its `deps-diff` alias).

## [1.4.2] - 2026-07-01

### Fixed
- **Progress indicator for `asset metadata create-batch`** - The `--progress` output previously used a hand-rolled carriage-return line that did not clear the previous message, so when a shorter asset path followed a longer one the leftover characters mangled the line (and error messages were printed directly onto the progress line). The command now renders a proper progress bar (spinner, elapsed time, position/total, current path) that redraws cleanly, matching the other batch commands.

### Changed
- **Cleaner error output for `asset metadata create-batch --continue-on-error`** - Unresolved asset paths are now reported as a single concise warning line per asset instead of repeating the full multi-step remediation block for every skipped row. The detailed guidance is shown once as a summary at the end of the run. Diagnostics are also emitted without corrupting the progress bar.

## [1.4.1] - 2026-06-30

### Fixed
- **`visual-match` pagination** - Visual search now returns the full ranked result set instead of only the first ~20 matches. The `visual-search` API endpoint takes `page`/`perPage` as **query parameters**, but the client was sending them in the request body (which the API ignores), so every call returned only the first page. The client now sends pagination in the query string and walks all pages. Affects `pcli2 asset visual-match` and `pcli2 folder visual-match` (and their `visual-search` aliases).

### Added
- **`--limit` for the visual-match commands** - Cap the number of results returned by `asset visual-match` and `folder visual-match`, defaulting to **100**. Visual search ranks every asset by visual similarity (there is no similarity threshold in the API), so a limit keeps the output manageable; pass a higher `--limit` to retrieve more. The client requests only as many results per page as needed, so a small limit costs a single API call.

## [1.4.0] - 2026-06-30

### Added
- **`--format xls` for `folder geometric-match`** - A new Excel (`.xlsx`) output format that renders the match report as a color-highlighted, human-friendly workbook. It contains exactly the same columns as the CSV output (always including the `REF_`/`CAN_` metadata pairs), with visual aids for scanning large reports: frozen header rows and identity columns, grouped/boxed `REF_`/`CAN_` metadata pairs, per-cell metadata diff highlighting (green match / red differ / amber missing-on-one-side), a heat-map gradient over `MATCH_PERCENTAGE` with rows sorted by match descending, and clickable `COMPARISON_URL` hyperlinks.
  - Because Excel is binary, `xls` writes to a file rather than stdout: use `--output`/`-o` to set the path (default `match_report.xlsx`); the extension is normalized to `.xlsx`, and a warning is printed to stderr if it had to be changed. On success the command prints nothing to stdout (UNIX convention).
  - The `xls` format always includes metadata, so `--metadata` is implied.
  - Ported from the standalone `match-report-analyzer` tool for a consistent look and feel.
  - Affects `pcli2 folder geometric-match` (and its `geometric-search` alias).

### Changed
- **`folder geometric-match`: `COMPARISON_URL` moved to the last column** - In both the CSV and Excel output, the long, rarely-read `COMPARISON_URL` column is now the final column, after the `REF_`/`CAN_` metadata columns (previously it came before them). Affects `pcli2 folder geometric-match` when run with `--metadata` (and the new `--format xls`, which always includes metadata).
- **Candidate metadata column prefix renamed `CAND_` → `CAN_`** - Match-report CSV output (and the new Excel output) now prefixes candidate-asset metadata columns with `CAN_` instead of `CAND_`, keeping it the same length as the `REF_` reference prefix for visual consistency and matching the `match-report-analyzer` convention. Affects the metadata columns of all match commands (`asset geometric-match`, `asset part-match`, `asset visual-match`, and their `folder` counterparts) when run with `--metadata`.

## [1.3.0] - 2026-06-30

### Added
- **`asset similarity` command** - Get the pairwise geometric (and, when enabled, volumetric) match scores between two specific assets, backed by the Physna `GetMatchScores` API endpoint. Unlike `asset geometric-match`, which searches the tenant for assets similar to one reference, this command compares two assets you already know.
  - Each asset is identified by **either** a UUID **or** a path, resolved to a UUID via the shared `resolve_asset` helper: `--reference-uuid`/`--reference-path` and `--candidate-uuid`/`--candidate-path` (each pair is mutually exclusive and required)
  - Output supports JSON (default) and CSV (with optional `--headers`), and includes a UI comparison URL, consistent with the other match commands
  - The `volumetric` score is included only when volumetric scoring is enabled for the tenant; otherwise it is omitted from JSON and left blank in CSV
  - Available under the alias `asset match-scores`
  - Affects `pcli2 asset similarity` (and its `match-scores` alias)

## [1.1.10] - 2026-05-07

### Added
- **`--override` flag for `asset create`** - When uploading an asset that already exists at the target path, automatically delete the existing asset and re-upload the new version in its place. Without this flag, a duplicate asset is created at the same path.
  - Proactively checks for an existing asset before uploading; if found, deletes it first
  - Affects `pcli2 asset create` (and its `upload` alias)
- **`--restore-metadata` flag for `asset create`** - When used together with `--override`, preserves the existing asset's metadata and applies it to the newly uploaded asset. The metadata is passed directly in the upload request for a single-operation restore.
  - Requires `--override` (enforced by the CLI parser)
  - If the existing asset has no metadata, the flag is silently ignored
  - Affects `pcli2 asset create` (and its `upload` alias)

## [1.1.8] - 2026-04-21

### Added
- **`--continue-on-error` flag for `asset metadata create-batch`** - Allows the batch to skip rows whose asset path cannot be resolved and continue processing remaining rows. Mirrors the flag already available on `folder download`, `folder upload`, and `folder thumbnail` commands.
  - Reuses the shared `continue_on_error_parameter()` helper in `commands::params` for consistency across commands
  - Affects `pcli2 asset metadata create-batch` (and its `update-batch` alias)

### Changed
- **`asset metadata create-batch` default error behavior** - By default, any error (unresolvable asset path or failed metadata API call) now terminates the batch with a summary of successes and failures printed to stderr. Previously, asset-path lookup failures and metadata API errors were logged but the batch silently continued.
  - Pass `--continue-on-error` to skip unresolvable asset paths and continue with the remaining rows
  - Metadata API errors (delete/update) always terminate execution regardless of the flag, because the API layer already retries transient HTTP failures internally
  - CSV parsing errors continue to terminate execution immediately, as before
  - Authentication failures continue to terminate execution with a remediation message, as before
  - Affects `pcli2 asset metadata create-batch` command

## [1.1.7] - 2026-04-13

### Changed
- **`asset metadata create-batch` empty value now deletes the field** - Previously, empty values in the CSV were silently skipped. Now an empty VALUE column triggers deletion of that metadata field from the asset, enabling true round-trip workflows where exporting, clearing a value, and reimporting removes the field.
  - The batch command now calls the dedicated delete API endpoint for empty-value rows before applying updates
  - Authentication and other errors during the delete step are handled with the same retry/break logic as updates
  - Affects `pcli2 asset metadata create-batch` command
- **Smarter type detection for untyped metadata values** - `asset metadata create` (single-asset) now auto-infers boolean and numeric types when `--type` is omitted or set to an unknown value, reducing type-mismatch errors when a field is already registered as `boolean` or `number` in Physna.
  - Recognises `true`/`false`/`yes`/`no` (case-insensitive) as booleans and integer/float strings as numbers
  - Falls back to `text` with sanitization for everything else

## [1.1.6] - 2026-04-13

### Fixed
- **Root folder path `"/"` resolution** - Fixed `FolderNotFound` error when specifying `--folder-path "/"` for commands such as `asset list`
  - `FolderHierarchy::get_folder_by_path` intentionally returns `None` for `"/"` (root is not a single node), but the caller was treating `None` as "not found"
  - Non-recursive `asset list --folder-path "/"` now correctly lists root-level assets (equivalent to omitting `--folder-path`)
  - Recursive `asset list --folder-path "/" --recursive` now correctly traverses all folders
  - Affects `pcli2 asset list` command
- **`asset metadata update-batch` token expiration handling** - Fixed issue where long-running batch operations would fail with authentication errors when the token expired mid-session
  - Added pre-flight token expiration check that warns if token may expire during batch operation
  - Added proactive token refresh before each asset is processed (refreshes if within 2 minutes of expiration)
  - Authentication errors now properly detected and batch operation stops immediately with clear remediation steps
  - Added success/failure summary at end of batch operation
  - Affects `pcli2 asset metadata update-batch` and `pcli2 asset metadata create-batch` commands
- **`asset metadata create-batch` empty value handling** - Fixed issue where CSV files with empty metadata values would fail with confusing error messages
  - Empty metadata values are now skipped during CSV parsing (API rejects empty strings)
  - Added debug logging to show which empty values were skipped
  - Improved error messages for 400 Bad Request errors to show actual API error instead of generic "Operation conflict"
  - Affects `pcli2 asset metadata create-batch` and `pcli2 asset metadata update-batch` commands

## [1.1.3] - 2026-03-16

### Fixed
- **Folder download error handling** - Complete overhaul of error handling and reporting for `folder download` command
  - Fixed incorrect error counts with concurrent downloads (`--concurrent` flag)
  - Progress bars no longer corrupted by inline error messages
  - All errors now collected and displayed AFTER statistics for visibility
  - Error messages now show actual API errors instead of generic messages
  - Always waits for all download tasks to complete before reporting
  - Folder cache invalidated at start to ensure fresh data from server
  - Stats summary now shown regardless of success or error
  - Detailed error list remains visible on screen at end of run
- **Inline logging during downloads** - Removed `tracing::error!()` and `tracing::warn!()` calls that corrupted progress bar display
  - All errors now collected in memory and printed once at completion
  - Cleaner user experience with no scrolling error messages during download

## [1.1.2] - 2026-03-14

### Fixed
- **Asset create path resolution** - Fixed bug where assets uploaded with `--folder-uuid` were placed in the root directory instead of the specified folder
  - When using `--folder-uuid` (instead of `--folder-path`), the asset path is now correctly constructed using the folder's actual path
  - Affects `pcli2 asset create` command
- **Case-insensitive folder path matching** - Fixed folder lookup to use case-insensitive comparison for cross-platform compatibility
  - Windows users can now use folder paths with any casing (e.g., `photos and models`, `PHOTOS AND MODELS`)
  - Matches Windows file system behavior where paths are case-insensitive
  - Affects all commands that accept `--folder-path`: `asset list`, `folder resolve`, `folder download`, etc.
  - Adds comprehensive test coverage for case-insensitive matching
- **Stale folder cache issue** - Fixed issue where deleted and recreated folders would return stale UUIDs from cache
  - Reduced default cache expiration from 24 hours to 1 hour to minimize stale data issues
  - Added `--reload` flag to `folder resolve` command to force cache refresh before resolving
  - Added `--reload` flag to `asset list` command to force cache refresh before listing
  - Users can now run `pcli2 folder resolve --reload --folder-path "..."` or `pcli2 asset list --reload --folder-path "..."` to bypass stale cache

### Added
- **New `cache clear` command** - Added dedicated command for clearing all caches on demand
  - `pcli2 cache clear` - Clear all caches (folder, metadata, tenant)
  - `pcli2 cache clear --folder` - Clear only folder cache
  - `pcli2 cache clear --metadata` - Clear only metadata cache
  - `pcli2 cache clear --tenant` - Clear only tenant cache
  - `pcli2 cache clear --yes` - Skip confirmation prompt
  - Aliases: `pcli2 cache clean` works the same as `pcli2 cache clear`

## [1.1.0] - 2026-03-09

### Added
- **Metadata type mismatch detection** - Proactive detection of incompatible metadata type updates before API calls
  - New `ApiError::MetadataTypeMismatch` error variant with detailed error message
  - Type checking compares existing field types with provided values
  - Clear error messages explain the field name, expected type, and provided type
  - Suggests remediation steps (use matching type or recreate field)
- **Type inference for JSON values** - Helper methods to determine JSON value types
  - `infer_json_value_type()` - Identifies text, number, boolean, null, array, and object types
  - `is_type_compatible()` - Validates type compatibility between field definitions and values
- **Enhanced error messages** - User-friendly error output for metadata type mismatches

### Changed
- **Improved metadata update workflow** - Type validation occurs before API requests
  - Prevents confusing 404 errors when updating metadata with incompatible types
  - Fetches and caches metadata field definitions during update operations
  - Returns specific error instead of generic "Resource not found"

## [1.0.0] - 2026-03-04

### Added
- **Top-level `environment` command** - Moved `config environment` to first-level `environment` (alias: `env`) for better ergonomics
- **Command aliases** - Added Unix-style aliases across all commands:
  - `tenant`: `ls`, `select`, `unset`
  - `folder`: `ls`, `rm`, `cat`, `add`, `mv`, `ren`, `res`, `dl`
  - `asset`: `ls`, `rm`, `cat`, `upload`, `dl`, `deps`, `thumb`
  - `auth`: `a`, `in`, `out`, `token`, `clear`, `exp`
- **Confirmation prompts** - Destructive operations (`asset delete`, `folder delete`) now prompt for confirmation
- **`--yes` / `-y` flag** - Skip confirmation prompts for scripting and CI/CD
- **`PCLI2_NO_COLOR` environment variable** - Disable color output for CI/CD and logs
- **`--no-color` flag** - Command-line flag to disable color output
- **`config validate` command** - Validate configuration, credentials, and optionally test API connectivity
- **Structured logging** - Support for `PCLI2_LOG_LEVEL` and `RUST_LOG` environment variables (error, warn, info, debug, trace)
- **Progress bar enhancements** - Added throughput display (items/second) and improved ETA formatting
- **Examples in --help** - Added comprehensive examples to main help output showing common workflows
- **Docker support** - Added Dockerfile and .dockerignore for containerized deployments
- **Homebrew formula** - Added Homebrew tap support for macOS/Linux installation
- **Benchmark suite** - Added criterion-based benchmarks for performance tracking
- **Enhanced CI workflow** - Multi-platform testing (Linux, macOS, Windows), fmt check, clippy linting, and coverage reporting

### Changed
- **Improved CI/CD** - GitHub Actions now runs on all major platforms with comprehensive quality checks
- **Better error handling** - Confirmation failures gracefully exit without errors

### Technical Details
- All changes are backward compatible
- No breaking changes to existing functionality
- 151+ tests passing
- Zero clippy warnings

## [0.2.35] - 2026-03-04

### Added
- Comprehensive regression test suite with 85 new tests
  - 41 tests for error types, conversions, and error handling utilities
  - 44 tests for format utilities, OutputFormat, and path normalization
  - Tests provide protection against regressions during future refactoring
- **Fuzzy path matching** for folder-not-found errors
  - Case-insensitive folder path detection
  - Levenshtein distance-based similarity scoring
  - "Did you mean?" suggestions for all folder commands
  - Works across `asset list`, `folder get`, `folder download`, `folder upload`, and more

### Changed
- **Major code reorganization** - Split large modules for better maintainability
  - Split `actions/assets.rs` (3,828 lines) into 8 focused modules
  - Extracted formatting implementations from `model.rs` into `format/impls/`
  - Reduced `model.rs` from 4,582 to 2,352 lines (-49%)
- **Format parameter handling consolidated** - All format parsing now uses `FormatParams::from_args()` for consistency
  - Updated 8 functions in `actions/assets.rs`
  - Removed ~100 lines of duplicated format parsing code
- **Improved error handling** - Replaced `.unwrap()` calls with proper error propagation
  - 5 `.unwrap()` calls in `cli.rs` replaced with `?` operator
  - Better error messages with actionable suggestions

### Fixed
- Folder not found errors now preserve helpful path suggestions instead of generic messages

### Technical Details
- 151 tests passing (from 64)
- Zero clippy warnings
- Net reduction of ~1,097 lines (-15%)
- No breaking changes - all changes are backward compatible

## [0.2.34] - 2026-03-03

### Fixed
- Assembly download now correctly preserves all files including the top-level assembly file (previously the assembly file was deleted during ZIP cleanup due to filename conflict)

## [0.2.31] - 2026-02-11

### Added
- `user list` command to list users in the current tenant with JSON and CSV output support
- `user get` command to get details for a specific user with JSON and CSV output support
- Proper pagination handling for user listing with safeguards against infinite loops

### Fixed
- Fixed async runtime interference caused by blocking operations in context module that was breaking folder resolution
- Restored proper functionality of folder resolution and asset listing operations
- Fixed issue where folder paths were not being found due to runtime conflicts

## [0.2.30] - 2026-02-10

### Added
- `asset list` command now supports a `--thumbnails` flag to include thumbnail information
- `folder thumbnail` command to download thumbnails for all assets in a folder
- Recursive asset listing using folder hierarchy approach
- `asset thumbnail` command now supports a `--file` parameter to specify output file path

### Changed
- Updated README.md to document the new --thumbnails flag for asset list command
- Improved error handling for missing thumbnails in folder thumbnail command
- Changed default output directory name for folder thumbnail downloads
- Changed the file parameter in asset thumbnail command from positional to named (--file)

### Fixed
- Various typos in documentation and help text
- Validation for output file path in asset thumbnail command to ensure parent directory exists

## [0.2.29] - 2026-02-08

### Added
- Enhanced tenant state command with optional --type parameter to filter assets by state
- Added support for filtering assets by states: indexing, finished, failed, unsupported, no-3d-data, missing-dependencies
- Implemented pagination handling for the ListAssetsByState API endpoint

## [0.2.28] - 2026-02-08

### Changed
- Applied multiple clippy fixes to improve code quality and maintainability
- Fixed collapsible if statements, useless conversions, and other code quality issues
- Updated README.md to reflect new features and code quality standards

## [0.2.27] - 2026-02-08

### Fixed
- Fixed --exclusive flag logic for folder geometric-match, part-match, and visual-match commands to properly check if both reference and candidate assets belong to specified folder paths
- Added path normalization to ensure accurate folder comparisons with the --exclusive flag

### Changed
- Updated README.md to document the behavior of the --exclusive flag

## [0.2.25] - 2026-02-06

### Fixed
- Fixed CSV record length issue in asset list with metadata flag that was causing panics

## [0.2.24] - 2026-02-05

### Added
- Added detailed statistics report to folder upload command matching the format used in folder download
- Added --reload flag to folder list command to clear cache and reload from server
- Added visible alias "upload" for "asset create" command for better user experience
- Enhanced error handling for unsupported file types with user-friendly messages

### Fixed
- Fixed issue where folder upload command would fail when Physna folder already existed instead of using existing folder
- Fixed cache staleness issue that prevented folder resolution when folder creation conflicted
- Fixed API parameter conflicts that caused 400 Bad Request errors during asset creation
- Fixed typo in concurrent upload progress message ("Starging" to "Uploading")
- Fixed progress bar handling for skipped assets in concurrent uploads to prevent accumulation of unfinished progress bars
- Fixed individual progress bar management in concurrent operations to prevent visual artifacts

### Changed
- Improved folder upload logic to check if folder exists, create if needed, or use existing folder
- Enhanced error messages for unsupported file types with specific guidance for users
- Updated README.md to document new upload statistics report and folder upload functionality

## [0.2.23] - 2026-02-05

### Added
- Added asset thumbnail command to download asset thumbnails from Physna
- Implemented thumbnail API method to call the /tenants/{tenantId}/assets/{assetId}/thumbnail.png endpoint
- Added command-line interface for thumbnail with --uuid and --path parameters
- Added constants for thumbnail command following existing patterns
- Updated README.md with documentation for the new thumbnail command
- Added folder thumbnail command to download thumbnails for all assets in a folder and its subfolders
- Implemented concurrent thumbnail download with progress tracking, error handling, and retry logic
- Added command-line interface for folder thumbnail with --progress, --concurrent, --delay, and --continue-on-error parameters
- Updated README.md with documentation for the new folder thumbnail command

## [0.2.22] - 2026-02-05

### Added
- Added asset thumbnail command to download asset thumbnails from Physna
- Implemented thumbnail API method to call the /tenants/{tenantId}/assets/{assetId}/thumbnail.png endpoint
- Added command-line interface for thumbnail with --uuid and --path parameters
- Added constants for thumbnail command following existing patterns
- Updated README.md with documentation for the new thumbnail command
- Added folder thumbnail command to download thumbnails for all assets in a folder and its subfolders
- Implemented concurrent thumbnail download with progress tracking, error handling, and retry logic
- Added command-line interface for folder thumbnail with --progress, --concurrent, --delay, and --continue-on-error parameters
- Updated README.md with documentation for the new folder thumbnail command

## [0.2.21] - 2026-02-04

### Fixed
- Cleaned up all compilation warnings
- Fixed doc comment formatting issues in src/physna_v3.rs
- Addressed large enum variant warnings with appropriate attributes
- Removed redundant assert!(true) statements from tests
- Fixed wildcard pattern in src/metadata.rs
- Fixed non-canonical partial_cmp implementation in src/model.rs
- Fixed box collection issue in src/model.rs
- Added #[allow] attributes to suppress legitimate warnings

## [0.2.20] - 2026-02-03

### Added
- Added asset reprocess command to trigger reprocessing of assets in the Physna system
- Implemented reprocess API method to call the /tenants/{tenantId}/assets/reprocess endpoint
- Added command-line interface for reprocess with --uuid and --path parameters
- Added constants for reprocess command following existing patterns
- Updated README.md with documentation for the new reprocess command

## [0.2.19] - 2026-01-29

### Added
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections
- Added short form (-n) for --name parameter in config environment commands
- Added comprehensive NuShell examples for advanced pipeline operations
- Added update instructions explaining different update methods based on installation approach
- Added proper statistics tracking for download operations

### Changed
- Improved folder download command to check for existing files before downloading when --resume flag is used
- Enhanced documentation with more detailed examples and parameter explanations
- Optimized resume functionality to apply delays only when actually downloading, not when skipping files
- Changed skip message log level from INFO to DEBUG in resume functionality

## [0.2.18] - 2026-01-29

### Added
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections

## [0.2.17] - 2026-01-28

### Added
- Implemented performance and efficiency optimizations across multiple components
- Added HTTP connection pooling and optimized timeout configuration
- Implemented lazy loading and caching for expensive computations
- Added streaming for large file operations and pagination
- Implemented better error handling for individual file failures in batch operations
- Added configurable concurrency limits for batch operations
- Implemented bulk operations to reduce API calls
- Added caching mechanisms to avoid redundant API calls

### Changed
- Optimized batch processing with improved progress tracking and detailed reporting
- Enhanced path resolution logic to reduce redundant API calls
- Improved memory usage with streaming implementations for large files
- Refactored code to reduce unnecessary cloning operations

## [0.2.16] - 2026-01-27

### Added
- Added detailed shell completion instructions for all supported shells (Zsh, Bash, Fish, PowerShell, Elvish) to README.md

### Changed
- Enhanced shell completion documentation with comprehensive setup instructions for each shell
- Updated README.md with step-by-step instructions for applying shell completions

## [0.2.15] - 2026-01-27

### Added
- Added --metadata flag to asset get command to include metadata in output for both CSV and tree formats
- Added --continue-on-error flag to folder download command to continue downloading other assets if one fails
- Added --concurrent flag to folder download command to allow multiple concurrent downloads (range: 1-10)
- Added --delay flag to folder download command to add delays between downloads (range: 0-180 seconds)
- Added enhanced error logging with asset UUID and Physna path information to folder download command
- Added improved progress indicators for concurrent downloads using MultiProgress
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections

### Changed
- Improved folder download command to show separate progress bars for each concurrent download when using --progress
- Enhanced error messages to include more specific information about which asset failed during download
- Updated documentation in README.md with examples of new flags and features

## [0.2.14] - 2026-01-25

### Added
- Added folder dependencies command to get dependencies for all assembly assets in one or more folders
- Added --progress flag to folder dependencies command for visual feedback during processing
- Added ASSEMBLY_PATH column to CSV output showing relative path within assembly hierarchy
- Added original_asset_path field to AssetDependency struct to track which original asset each dependency belongs to
- Added consistent state normalization converting "missing-dependencies" to "missing" for all formats

### Changed
- Moved match-folder commands from asset namespace to folder namespace: folder geometric-match, folder part-match, folder visual-match
- Renamed folder-based commands from geometric-match-folder to geometric-match (and similar for part-match, visual-match)
- Updated asset dependencies command to always operate recursively by default (removed --recursive flag)
- Improved CSV output sorting by ASSET_PATH then ASSEMBLY_PATH for consistent ordering
- Changed command structure to be more intuitive: folder dependencies instead of asset dependencies with folder paths
- Updated tree format to show state information alongside asset names
- Modified CSV output to show "None" instead of nil UUID when UUID is not available
- Improved assembly path handling to show proper hierarchy in all output formats

### Fixed
- Fixed issue where only first level of dependencies was shown instead of full hierarchy
- Fixed CSV output to properly show assembly hierarchy paths
- Fixed state display consistency across all output formats (tree, JSON, CSV)

## [0.2.13] - 2026-01-24

### Added
- Added text-match command with text-search alias to perform text-based asset searches
- Added --fuzzy flag to control search behavior (default: false for exact search with quoted text)
- Added TextMatch, TextSearchResponse, EnhancedTextSearchResponse, and TextMatchPair data models
- Added text_search API method to PhysnaApiClient for calling the text-search endpoint
- Added ASSET_NAME, TYPE, STATE, IS_ASSEMBLY, RELEVANCE_SCORE, ASSET_UUID, and ASSET_URL columns to CSV output
- Added aliases for existing match commands: geometric-search, part-search, visual-search
- Added folder variants for new aliases: geometric-search-folder, part-search-folder, visual-search-folder

### Changed
- Updated README.md with comprehensive documentation for text-match command and new aliases
- Modified text search to wrap queries in quotes by default for exact matching
- Improved asset URL construction to avoid duplicate tenant paths in URLs
- Enhanced CSV output format with additional asset metadata fields

## [0.2.12] - 2026-01-24

### Fixed
- Fixed token refresh mechanism to save refreshed tokens to keyring immediately after refresh in all API request methods
- Ensured subsequent commands use fresh tokens instead of expired ones after automatic refresh
- Resolved compilation warnings related to unused functions and imports
- Fixed AssemblyTree OutputFormatter implementation to properly handle all output formats (JSON, CSV, Tree)

### Changed
- Added token saving calls to all API request execution methods after successful refresh
- Improved error handling for token refresh operations across all API methods
- Consolidated token refresh and save logic for better reliability

## [0.2.11] - 2026-01-24

### Fixed
- Fixed token refresh mechanism to save refreshed tokens to keyring immediately after refresh
- Ensured subsequent commands use fresh tokens instead of expired ones after automatic refresh
- Resolved compilation warnings related to unused functions and imports
- Fixed AssemblyTree OutputFormatter implementation to properly handle tree format output

### Changed
- Moved token saving logic to refresh_token method for immediate persistence of new tokens
- Added token saving calls to all API request methods after successful refresh
- Improved error handling for token refresh operations

### Added
- Added 'pcli2 auth expiration' command to check access token validity and show expiration time in local time zone
- Implemented JWT decoding functionality to extract expiration claims from access tokens
- Added human-readable time remaining display (e.g., "59m 14s") for token expiration

### Fixed
- Fixed asset dependencies command to properly handle assets with no dependencies (was showing authentication error)
- Fixed token refresh mechanism to skip failing refresh attempts and guide users to re-authenticate properly
- Improved error handling for 404 responses when assets have no dependencies
- Added proper handling of NotFoundError in API response processing

### Changed
- Modified refresh_token method to attempt automatic re-authentication using cached credentials
- Updated HTTP error handling to distinguish between authentication issues and missing resources
- Enhanced AssemblyTree and AssemblyNode formatting with proper tree characters for better visualization
- Improved tree format output using proper Unicode box-drawing characters (├──, └──, │)
- Modified 'pcli2 auth expiration' command to display expiration time in user's local time zone instead of UTC

## [0.2.9] - 2026-01-20

### Fixed
- Fixed creation of folders under root
- Fixed folder move operations to properly support moving to root level
- Improved path normalization for folder operations

## [0.2.8] - 2026-01-19

### Fixed
- Fixed AssetStateCounts deserialization to match actual API response format
- Made all AssetStateCounts fields optional to handle missing values gracefully
- Renamed AssetStateCounts fields to match API response (indexing, finished, failed, unsupported, no-3d-data)
- Updated CSV headers to match JSON field names for consistency in tenant state command
- Updated documentation for tenant state command with detailed usage examples

### Added
- Added comprehensive documentation for tenant state command in README.md
- Added detailed descriptions for each asset state field (indexing, finished, failed, unsupported, no-3d-data)

## [0.2.7] - 2026-01-17

### Added
- Added resolve-folder command to resolve a folder path to its UUID
- Added comprehensive test suite for path normalization with edge cases
- Added assets_count and folders_count fields to Folder model for richer folder information
- Added 'issues/' directory to .gitignore
- Added completions command to generate shell completions for bash, zsh, fish, powershell, and elvish
- Added documentation for shell completions setup

### Changed
- Improved normalize_path function to handle consecutive slashes and collapse them into single slashes
- Modified folder list command to show only direct children for non-tree formats (previously showed all descendants)
- Enhanced error handling in API client to extract meaningful error messages from response bodies
- Updated folder move command to properly handle root path cases
- Modified list_folders to use get_children_by_path for non-tree formats
- Enhanced CSV output for folders to include PATH, ASSETS_COUNT, and FOLDERS_COUNT fields
- Improved documentation for folder rename and move commands with comprehensive examples
- Updated README.md and quickstart guide to reflect changes in folder command behavior
- Made -p a consistent short form for --folder-path parameter across all commands
- Improved error messages to provide clearer guidance when no tenant is selected
- Updated environment switching message to be more informative about tenant selection
- Fixed tenant cache to be environment-specific, resolving issues when switching environments
- Added tenant state command to get asset state counts (processing, ready, failed, deleted)
- Added AssetStateCounts model to represent asset state counts
- Implemented API call to retrieve asset state counts from the API

## [0.2.6] - 2026-01-16

### Added
- Added visual-match and visual-match-folder commands for finding visually similar assets
- Added comprehensive documentation for visual matching functionality
- Added concurrent processing option for visual-match-folder with configurable limits (1-10)
- Added progress tracking with multi-progress bars for visual-match-folder operations
- Added exclusive flag for visual-match-folder to limit matches to specified folders only
- Added metadata support for visual matching with REF_ and CAND_ prefixes in CSV output

## [0.2.5] - 2026-01-14

### Added
- Added part-match and part-match-folder commands for finding part matches between assets
- Added comprehensive documentation for part matching functionality
- Added concurrent processing option for part-match-folder with configurable limits (1-10)
- Added progress tracking with multi-progress bars for part-match-folder operations
- Added exclusive flag for part-match-folder to limit matches to specified folders only
- Added proper comparison URLs for part matching results in both JSON and CSV formats
- Added metadata support for part matching with REF_ and CAND_ prefixes in CSV output

## [0.2.4] - 2026-01-14

### Added
- Added comprehensive uninstallation instructions to README.md for Windows, macOS, and Linux platforms
- Enhanced CSV output for geometric-match and part-match commands to properly include comparison URLs
- Added proper metadata support for geometric-match and part-match commands with REF_ and CAND_ prefixes

### Fixed
- Fixed CSV header consistency issues in EnhancedPartSearchResponse to include COMPARISON_URL field
- Fixed CSV output formatting to ensure consistent field counts across all records
- Fixed metadata loading for reference assets in geometric-match and part-match commands
- Removed failing integration tests that require API access from CI environment

## [0.2.3] - 2024-01-09

### Added
- Added `--refresh` flag to `context set tenant` command to force fetching fresh tenant list from API
- Implemented comprehensive tenant caching system with 1-hour TTL
- Added tenant cache persistence to disk for improved performance
- Added PCLI2_FORMAT environment variable support for default output format
- Added PCLI2_HEADERS environment variable support for default CSV header inclusion
- Added comprehensive documentation for system keyring integration and security
- Added documentation for environment variable precedence and usage

### Changed
- Reverted to dev-keyring as default for broader platform compatibility (was system keyring)
- Improved error message formatting to remove raw technical data dumps
- Removed redundant context messages from error output for cleaner user experience
- Updated documentation to prevent automatic GitHub release detection
- Enhanced credential storage to support environment-specific credentials
- Added PCLI2_FORMAT and PCLI2_HEADERS environment variable support with proper precedence
- Improved error messages for authentication failures to be more user-friendly
- Enhanced error handling for common API error responses
- Reorganized documentation sections by moving "Geometric Matching", "Working with Metadata", and "Metadata Inference" to "Basic Usage"
- Made "Metadata Inference" a subsection under "Working with Metadata" for better organization
- Simplified Quick Start instructions by removing references to OpenAPI Documentation page for service account creation
- Clarified folder delete behavior with and without --force flag in documentation
- Moved "Best Practices" section to end of Geometric Matching section where it belongs
- Added comprehensive documentation section about using UNIX pipes with PCLI2 for advanced data processing

## [0.2.2] - 2024-01-09

### Fixed
- Improved error messages with consistent formatting and user-friendly guidance
- Added fun emojis to error messages for better visual indication
- Enhanced error remediation with specific steps for users

## [0.2.1] - 2024-01-08

### Fixed
- Fixed tenant parameter issue in `asset metadata delete` command that was causing "Mismatch between definition and access of tenant" error
- Fixed metadata delete command to use the proper API endpoint for deleting specific metadata fields from assets instead of fetching all metadata and re-updating the asset
- Improved error handling and documentation for metadata operations

## [0.2.0] - 2026-01-08

### Added
- Multi-environment configuration support with named environments
- `config environment add` command to add new environment configurations with custom URLs
- `config environment use` command with interactive selection to switch between environments
- `config environment list` command with detailed output and format options (json/csv)
- `config environment get` command to retrieve specific environment details with format options
- `config environment remove` command to delete environment configurations
- `config environment reset` command to reset all environment configurations to blank state
- Environment-specific URL configuration (API, UI, Auth) for different Physna instances
- Support for development, staging, and production environment configurations
- Active tenant clearing when switching environments to prevent cross-environment confusion
- Format options (json/csv) with headers and pretty printing for environment commands
- Comprehensive documentation for multi-environment configuration management

### Changed
- Authentication now uses environment-specific URLs instead of hardcoded production URLs
- OAuth2 client credentials flow now includes proper Content-Type header
- Enhanced error handling and tracing for authentication flows
- Updated README.md with comprehensive multi-environment configuration documentation
- Improved cross-platform configuration with environment variable support for custom URLs
- Restructured environment command hierarchy for better usability
- Limited environment command format options to json/csv (removed tree format where inappropriate)

### Fixed
- Hardcoded production URLs replaced with configurable environment-specific URLs
- Authentication flow now properly uses configuration-based URLs
- Cross-environment tenant conflicts resolved by clearing active tenant on environment switch
- OAuth2 client credentials flow compliance with proper header requirements

## [0.1.8] - 2025-10-31

### Added
- Asset dependencies command to retrieve component relationships and referenced assets for assemblies
- Recursive dependency traversal with `--recursive` flag to show full assembly hierarchies
- Enhanced JSON output with `parentPath` field to preserve parent-child relationships in recursive mode
- Enhanced CSV output with `PARENT_PATH` column to preserve parent-child relationships in recursive mode
- Hierarchical tree visualization for recursive dependencies showing proper indentation structure
- Cycle detection and deduplication to prevent infinite loops during recursive traversal
- Consistent parameter handling with `PARAMETER_RECURSIVE` constant for reuse across commands

### Changed
- Improved tree format for asset dependencies to show original asset as root instead of generic "Asset Dependencies"
- Updated API client to use asset path instead of asset ID in dependencies endpoint URL as per API specification
- Enhanced error handling for invalid folder paths in folder list command to return appropriate error messages
- Standardized command structure with consistent parameter naming and help text

### Fixed
- Asset dependencies endpoint URL construction to use asset path instead of asset ID as required by API
- Folder list command to return error for invalid paths instead of showing all root folders
- Banner display consistency to show for both `help` subcommand and `--help` flag
- Compilation warnings and improved code safety throughout the codebase

## [0.1.7] - 2025-10-29

### Added
- Folder get command with UUID and path parameters support
- Debug tracing for folder path resolution to diagnose ETRACE folder issues

### Fixed
- Compilation errors in folder get command implementation
- Folder list issues related to ETRACE functionality

## [0.1.6] - 2025-10-29

### Changed
- Updated CI workflow configurations
- Fixed JSON parsing errors in CI pipeline
- Updated cargo-dist runner list
- Updated Cargo.lock dependencies

## [0.1.5] - 2025-10-29

### Added
- Metadata inference command implementation
- Geometric search pagination fixes
- Visible aliases for metadata commands: 'update' for 'create' and 'update-batch' for 'create-batch'

### Changed
- Improved code safety and usability by fixing unsafe unwraps
- Enhanced tenant resolution functionality
- Updated cargo-dist configuration with proper GUID/UUID for CI pipeline
- Reordered asset subcommands alphabetically in CLI help output

### Fixed
- Invalid tenant CRUD operations removal
- Cargo-dist configuration issues and eliminated unused manifest key warnings

## [0.1.4] - 2025-10-29

### Changed
- Fixed tenant resolution in multiple commands (asset get, asset delete, folder get, folder list)
- Fixed Cargo.toml configuration warnings
- Improved banner with better gradient colors and spacing
- Updated documentation and dependencies

## [0.1.3] - 2025-10-29

### Added
- Asset metadata get command to CLI structure
- Workflow to automatically update 'latest' tag on release
- Comprehensive metadata documentation

### Changed
- Improved banner gradient colors and spacing
- Added visible aliases for metadata commands
- Reordered asset subcommands alphabetically in CLI help output

## [0.1.2] - 2025-10-29

### Changed
- Updated documentation structure for GitHub Actions deployment
- Fixed broken documentation links by ensuring proper mdBook file structure
- Improved README.md structure and user flow
- Updated oranda.json with Apache-2.0 license identifier

## [0.1.1] - 2025-10-13

### Added
- Cross-platform directory structure support using standard platform directories
- Automatic directory creation for configuration, cache, and data files
- Comprehensive documentation for data storage locations on all platforms
- Environment variable support for customizing directory locations
- Duplicate filtering for geometric matching results to remove reciprocal asset pairs
- Proper sorting of geometric matching results by match percentage (descending)
- LICENSE file accessibility fix for documentation website
- WSL installation test script for verifying installation on Windows Subsystem for Linux

### Changed
- Updated configuration to use proper cross-platform directories from `dirs` crate
- Improved error handling for first-time users with automatic configuration creation
- Enhanced documentation with cross-platform installation and usage instructions
- Updated Oranda configuration for proper GitHub Pages deployment
- Reworked README as single self-contained document to fix broken links

### Fixed
- Broken LICENSE links in documentation website that resulted in 404 errors
- Duplicate asset pairs in geometric matching results (A→B and B→A)
- Confusing error messages for first-time users trying to run PCLI2
- Inconsistent licensing information between README.md and LICENSE file
- Compilation warnings related to unused variables
- Documentation website CSS styling issues on GitHub Pages

## [0.1.0] - 2023-01-17

### Added
- Initial release of PCLI2
- Basic asset management commands (create, list, get, delete)
- Folder management commands (create, list, get, delete, update)
- Tenant management functionality
- Authentication with OAuth2 client credentials flow
- Configuration management
- Context management for multi-tenant support

[Unreleased]: https://github.com/physna/pcli2/compare/v0.2.20...HEAD
[0.2.20]: https://github.com/physna/pcli2/compare/v0.2.19...v0.2.20
[0.2.19]: https://github.com/physna/pcli2/compare/v0.2.18...v0.2.19
[0.2.18]: https://github.com/physna/pcli2/compare/v0.2.17...v0.2.18
[0.2.17]: https://github.com/physna/pcli2/compare/v0.2.16...v0.2.17
[0.2.16]: https://github.com/physna/pcli2/compare/v0.2.15...v0.2.16
[0.2.15]: https://github.com/physna/pcli2/compare/v0.2.14...v0.2.15
[0.2.14]: https://github.com/physna/pcli2/compare/v0.2.13...v0.2.14
[0.2.13]: https://github.com/physna/pcli2/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/physna/pcli2/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/physna/pcli2/compare/v0.2.9...v0.2.11
[0.2.9]: https://github.com/physna/pcli2/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/physna/pcli2/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/physna/pcli2/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/physna/pcli2/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/physna/pcli2/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/physna/pcli2/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/physna/pcli2/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/physna/pcli2/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/physna/pcli2/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/physna/pcli2/compare/v0.1.8...v0.2.0
[0.1.8]: https://github.com/physna/pcli2/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/physna/pcli2/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/physna/pcli2/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/physna/pcli2/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/physna/pcli2/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/physna/pcli2/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/physna/pcli2/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/physna/pcli2/compare/v0.1.0...v0.1.1
