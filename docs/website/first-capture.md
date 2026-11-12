# Catch your first request

The fastest way to learn Snurpo is its built-in local fixture. It sends real requests through the capture engine, so you can inspect the whole path without configuring another app.

## Try the local traffic

1. Open the native Snurpo app.
2. Select **Generate test traffic** in the empty inspector, or the lightning bolt beside **New session**. Its tooltip is **Generate real test traffic through the proxy**.
3. Select a captured row. Switch between **Request** and **Response**, then **Body**, **Headers** and **Timing**.
4. Open **Analysis**. Follow an observation using **Inspect a request**.
5. Select **Stop capture**, quit Snurpo and reopen it. The saved session remains available.

The fixture runs on `127.0.0.1:17891` and sends six requests through the proxy, normally `127.0.0.1:17890`. It includes an intentional sign-in failure, repeated product calls and a slow endpoint. An HTTP 401 in this fixture is expected.

If a matching pause rule or global breakpoint is enabled, disable it before generating the automatic batch. Use your own client when you want to [pause a request interactively](rules-breakpoints.md).

## Connect your own client

Create a session with **New session**, then select **Start capture**. **Connection settings** shows the current listener address and port. Configure your HTTP client to use that proxy:

```sh
curl --proxy http://127.0.0.1:17890 http://example.com
```

This command sends a real request to example.com. Replace the proxy port if you changed it in Snurpo. The listener binds to localhost; it is not a proxy exposed to your whole network.

Select the new exchange and compare what your client sent with what the server returned. For encrypted traffic, continue with [HTTPS setup](https.md).

## Keep the investigation readable

Use **Session details** beside the session name to add a useful name, notes, tags or a pin. Capture is autosaved in the local archive. **Stop capture** stops listening; it does not delete the session.

Next: [find related requests](inspect-and-search.md), [replay one](replay-curl.md), or [export the session](sessions-export.md).
