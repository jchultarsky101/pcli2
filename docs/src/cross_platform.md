## Cross-Platform Configuration

PCLI2 supports cross-platform environments through environment variables, which is especially useful for WSL users running Windows executables or users who want to customize where configuration and cache files are stored.

### Environment Variables

```bash
# Set custom configuration directory (cross-platform support)
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Set custom cache directory (for all cache files)
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Set custom API, UI, and Auth URLs (overrides configuration)
export PCLI2_API_BASE_URL="https://custom-api.example.com/v3"
export PCLI2_UI_BASE_URL="https://custom-ui.example.com"
export PCLI2_AUTH_BASE_URL="https://custom-auth.example.com/oauth2/token"

# Useful for WSL users running Windows executables
export PCLI2_CONFIG_DIR="/home/$USER/.pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.pcli2/cache"
```

Environment Variable Details:
- `PCLI2_CONFIG_DIR`: Custom directory for configuration file (`config.yml`). If not set, uses the system's default configuration directory.
- `PCLI2_CACHE_DIR`: Custom directory for all cache files (asset cache, metadata cache, folder cache). If not set, uses the system's default cache directory.
- `PCLI2_API_BASE_URL`: Custom API base URL (overrides configuration). If not set, uses the configured or default API URL.
- `PCLI2_UI_BASE_URL`: Custom UI base URL (overrides configuration). If not set, uses the configured or default UI URL.
- `PCLI2_AUTH_BASE_URL`: Custom authentication URL (overrides configuration). If not set, uses the configured or default auth URL.

These environment variables allow PCLI2 to work seamlessly across different operating systems and environments, including:
- Windows (native)
- macOS (native)
- Linux (native)
- Windows Subsystem for Linux (WSL)
- Docker containers
- CI/CD environments