## Cross-Platform Configuration

PCLI2 reads a small set of environment variables. They are useful for WSL users
running a Windows executable, for CI jobs, and for anyone who wants configuration
and cache files somewhere other than the platform defaults.

```bash
# Where config.yml and the credentials file live
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Where the folder, tenant and metadata caches live
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Useful for WSL users running Windows executables
export PCLI2_CONFIG_DIR="/home/$USER/.pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.pcli2/cache"
```

| Variable | Effect |
|----------|--------|
| `PCLI2_CONFIG_DIR` | Directory holding `config.yml` and `dev_credentials.json`. Default: the platform configuration directory (`pcli2 config get path` prints it). |
| `PCLI2_CACHE_DIR` | Directory for all cache files. Default: the platform cache directory. |
| `PCLI2_FORMAT` | Default `--format` when the flag is not given on the command line. |
| `PCLI2_HEADERS` | Default `--headers` when the flag is not given (`1`/`0`, `yes`/`no`). |
| `PCLI2_LOG_LEVEL` | Log level: `error`, `warn` (default), `info`, `debug`, `trace`. `RUST_LOG` takes precedence when set. |
| `PCLI2_TIMEOUT` | Total request timeout in seconds (default 1800, to allow very large transfers). Connections time out after 15 seconds and a read after 300 seconds of silence regardless. |
| `PCLI2_MAX_RETRIES` | Retries for transient failures: connection errors, 408/429/502/503/504 (default 2; `0` disables). |
| `PCLI2_NO_COLOR`, `NO_COLOR` | Disable colored output. |
| `PCLI2_NO_UPDATE_CHECK`, `CI` | Disable the once-a-day new-version hint. |

API, UI and authentication URLs are not read from the environment. They belong to
an environment definition: `pcli2 env add --name staging --api-url ...`, then
`pcli2 env use --name staging`.

Paths are the same on every platform: `/Home/Parts/Bracket.stl`, where `/Home`
(the name Physna shows for the root folder) is optional. Folder path matching is
case-insensitive.
