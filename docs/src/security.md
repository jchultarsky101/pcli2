# Credentials and Security

## Where credentials are kept

`pcli2 auth login` stores, per environment, the **client ID**, the **client
secret** and the current **access token** in `dev_credentials.json`, next to
`config.yml` in the configuration directory (`pcli2 config get path` prints it;
`PCLI2_CONFIG_DIR` moves it).

The file is **plain text**. It is not encrypted; its protection is its
permissions:

- On macOS and Linux it is created readable and writable by your account only
  (`0600`), and an older file with looser permissions is tightened the next time
  pcli2 reads it.
- On Windows it inherits the permissions of the configuration directory, which
  is in your user profile.

Treat it like an SSH key: do not commit it, do not copy it to shared drives, and
do not bake it into container images.

The file is written atomically (a temporary file renamed into place) under a
lock, so several pcli2 runs at once cannot corrupt it. If it ever cannot be read,
pcli2 keeps it as `dev_credentials.json.unreadable-<time>` rather than
overwriting it; log in again, and recover the old file by hand if you need to.

Building pcli2 from source with `--no-default-features --features os-keyring`
stores the credentials in the operating system's keychain instead.

## Logging out

`pcli2 auth logout` (and `pcli2 auth clear-token`) removes the **access token**
only. The client ID and secret stay, so the next `pcli2 auth login` does not ask
for them again. To remove the secret from a machine, delete
`dev_credentials.json` (or the whole configuration directory).

## Keeping the secret out of history

`pcli2 auth login` without flags prompts for the client ID and secret, with the
secret masked. For scripts and CI, put them in `PCLI2_CLIENT_ID` and
`PCLI2_CLIENT_SECRET`, filled from your CI system's secret store. Avoid
`--client-secret` on the command line: it ends up in your shell history and is
visible to other users in process listings while pcli2 runs.

## What leaves your machine

- Requests go to the environment's API and authentication URLs (by default
  `app-api.physna.com` and Physna's Amazon Cognito endpoint) over HTTPS. The
  client secret is only ever sent to the authentication URL.
- Once a day, in an interactive terminal, pcli2 asks
  `api.github.com` for the latest release to tell you about updates. Set
  `PCLI2_NO_UPDATE_CHECK=1` to turn that off; it is off in CI.
- Nothing else: pcli2 collects no telemetry.

`pcli2 env add` accepts `http://` URLs (for a local test server) but warns, since
the secret and every token would then cross the network unencrypted.

Debug logs (`--verbose`, `PCLI2_LOG_LEVEL=debug`) never print the secret or the
access token.
