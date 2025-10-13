## Cross-Platform Configuration

PCLI2 supports cross-platform environments through environment variables, which is especially useful for WSL users running Windows executables or users who want to customize where configuration and cache files are stored.

### Environment Variables

```bash
# Set custom configuration directory (cross-platform support)
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Set custom cache directory (for all cache files)
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Useful for WSL users running Windows executables
export PCLI2_CONFIG_DIR="/home/$USER/.pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.pcli2/cache"
```

Environment Variable Details:
- `PCLI2_CONFIG_DIR`: Custom directory for configuration file (`config.yml`). If not set, uses the system's default configuration directory.
- `PCLI2_CACHE_DIR`: Custom directory for all cache files (asset cache, metadata cache, folder cache). If not set, uses the system's default cache directory.

These environment variables allow PCLI2 to work seamlessly across different operating systems and environments, including:
- Windows (native)
- macOS (native) 
- Linux (native)
- Windows Subsystem for Linux (WSL)
- Docker containers
- CI/CD environments