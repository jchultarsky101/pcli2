# Proxies, Certificates and Timeouts

## Proxies

pcli2 uses the standard proxy environment variables:

```bash
export HTTPS_PROXY=http://proxy.example.com:3128
export NO_PROXY=localhost,127.0.0.1
pcli2 doctor
```

`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` are honoured (upper or
lower case). On macOS and Windows the system proxy settings are used when no
variable is set.

## Certificates (corporate TLS inspection)

pcli2 uses the operating system's TLS stack and trust store: the Keychain on
macOS, the certificate store on Windows, OpenSSL and the system CA bundle on
Linux. A corporate root certificate that your browsers already trust is
therefore trusted by pcli2 too; there is nothing to configure in pcli2. On Linux,
add the certificate to the system bundle (`update-ca-certificates` or
`update-ca-trust`).

## Hosts to allow through a firewall

- The environment's API URL (default `https://app-api.physna.com`).
- The environment's authentication URL (default Physna's Amazon Cognito
  endpoint, `https://physna-app.auth.us-east-2.amazoncognito.com`).
- Downloads come from the API URL as well.
- Optional: `https://api.github.com` for the once-a-day update hint
  (`PCLI2_NO_UPDATE_CHECK=1` turns it off).

`pcli2 env get` shows the URLs of the active environment.

## Timeouts and retries

| Setting | Default | Change with |
|---|---|---|
| Connecting | 15 seconds | - |
| No data received on an open connection | 5 minutes | - |
| Whole request (large uploads and downloads) | 30 minutes | `PCLI2_TIMEOUT` (seconds) |
| Retries of transient failures | 2 | `PCLI2_MAX_RETRIES` (`0` disables) |

Transient failures are connection errors, rate limiting (429), 408 and 503, and
for requests that are safe to repeat also 502 and 504 and timeouts. A download
whose connection drops mid-file is started again. A request that keeps failing
after the retries exits 69 (try again later); see
[Scripting and Automation](scripting.md#exit-codes).

`pcli2 doctor` checks connectivity to both URLs and reports what it found.
