# Troubleshooting

Start with `pcli2 doctor`: it checks the binary, the configuration, the
credentials, the token, the tenant, the caches and connectivity in one screen,
and says what to do about anything that is wrong.

## By exit code

Every failure exits with a code that names its class (the full table is in
[Scripting and Automation](scripting.md#exit-codes)).

| Code | Meaning | What to do |
|---|---|---|
| 64 | The command line is wrong, or a prompt was needed with `--no-input` | Read the message; `--help` on the command shows its flags and examples |
| 65 | Data could not be read (a malformed CSV, an unexpected API answer) | Check the input file; for an API answer, run with `--verbose` and report it |
| 66 | An input file cannot be opened | Check the path and its permissions |
| 67 | An asset, folder, tenant or environment does not exist | Check the spelling; paths start at `/` (Physna shows the root as "Home", and `/Home/...` works too) |
| 68 | The deployment lacks a feature (failure diagnostics) or `doctor` could not reach a server | See [Proxies, Certificates and Timeouts](network.md) |
| 69 | Try again later: rate limited, a server error that outlasted the retries, a batch with failed items, a report still running | Run again later; for a batch, the summary lists what failed |
| 78 | The configuration is broken | `pcli2 config validate`, then `pcli2 env list` |
| 100 | Not logged in, or the credentials were rejected | `pcli2 auth login` |
| 101 | The network failed | See [Proxies, Certificates and Timeouts](network.md) |
| 102 | The API rejected the request | The message carries the server's reason |

## Common situations

**"Access token not found"**: run `pcli2 auth login`. In CI, set
`PCLI2_CLIENT_ID` and `PCLI2_CLIENT_SECRET` first.

**"No tenant specified and no active tenant selected"**: run `pcli2 tenant use`
(or pass `--tenant` to the command).

**A folder path is "not found" but exists**: the folder cache may predate the
folder; add `--reload` to `folder list` or `asset list`, or run
`pcli2 cache clear`.

**An asset shows `failed`**: `pcli2 asset diagnose --path <path>` asks Physna why;
`pcli2 tenant failures` lists what failed recently.

**Bulk commands are slow or rate limited**: folder commands run four items at a
time by default; lower it with `--concurrent 1` or space items out with
`--delay`.

**Seeing what pcli2 does**: `--verbose` prints pcli2's own debug log on stderr;
`RUST_LOG=debug` opens up the HTTP stack as well. `--stats` prints the number of
API requests, retries and token renewals at the end.

## Checking the basics

```bash
pcli2 auth expiration          # logged in, and for how long
pcli2 tenant get               # the active tenant
pcli2 env get                  # the active environment and its URLs
pcli2 config get path          # where the configuration lives
pcli2 config validate --api    # configuration, credentials and a test call
```

## Reporting a problem

Include the output of `pcli2 --version` and `pcli2 doctor`, the command you
ran, and the message (with `--verbose` if you can reproduce it). An internal
error (exit 70) is always a bug.
