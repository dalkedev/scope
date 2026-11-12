# Inspect HTTPS

HTTPS inspection needs two things: your client must route through Snurpo, and it must trust Snurpo's local certificate authority. A proxy setting alone is not enough.

## Connect a test client

1. Create or select a session and start capture.
2. Open **Connection settings**.
3. Under **Certificate authority**, use **Copy certificate path**.
4. Configure your client to trust that public certificate and use the displayed proxy address.

For cURL, replace the certificate placeholder with the actual copied path:

```sh
curl --proxy http://127.0.0.1:17890 \
  --cacert "/absolute/path/from/snurpo/scope-ca.pem" \
  https://example.com
```

The path is installation-specific. Keep it quoted if it contains spaces. TLS verification remains enabled; `--insecure` is not part of this setup.

## Public certificate, private signing key

Snurpo creates a local CA and keeps its signing key in the application data directory. Configure clients with the public `scope-ca.pem` certificate only. Do not distribute `scope-ca-key.pem` or include it in a bug report.

These legacy names are intentional compatibility identifiers. An existing CA can also display **Scope Local Debug CA**. Updating to Snurpo does not require deleting or regenerating it.

## When trust is not enough

An application can ignore the system proxy, use its own trust store, enforce certificate pinning, or send traffic over an unsupported protocol. An empty capture does not prove that the application made no request.

| Symptom | Check |
| --- | --- |
| No exchange appears | Confirm the client actually uses the configured HTTP proxy. |
| Certificate validation fails | Check the copied CA path and the client's selected trust store. |
| cURL works but an app fails | Check app-specific trust, pinning and networking stack. |
| An Android app rejects HTTPS | Follow the [Android guide](android.md); system proxy mode does not install a system CA or bypass pinning. |

Snurpo currently does not support HTTP/3 or WebSocket frame inspection. Use [troubleshooting](troubleshooting.md) to separate connection, trust and capture problems.
