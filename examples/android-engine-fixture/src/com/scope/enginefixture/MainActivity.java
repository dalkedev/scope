package com.scope.enginefixture;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URL;
import java.security.KeyStore;
import java.security.cert.CertificateFactory;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLContext;
import javax.net.ssl.TrustManager;
import javax.net.ssl.TrustManagerFactory;
import javax.net.ssl.X509TrustManager;
import okhttp3.CertificatePinner;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import org.json.JSONObject;

/** Owned synthetic-only lab client. Each explicit action makes one finite request. */
public final class MainActivity extends Activity {
    private static final AtomicBoolean busy = new AtomicBoolean(false);
    private static final AtomicInteger sequence = new AtomicInteger();
    private EditText input;
    private TextView result;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        input = new EditText(this);
        input.setSingleLine(true);
        input.setText("https://127.0.0.1:18443/");
        layout.addView(input);
        for (final String client : new String[] {"platform", "okhttp", "okhttp-scope"}) {
            Button button = new Button(this);
            button.setText("One request: " + client);
            button.setOnClickListener(new View.OnClickListener() {
                @Override public void onClick(View view) { request(client, input.getText().toString()); }
            });
            layout.addView(button);
        }
        result = new TextView(this);
        result.setText("Owned loopback fixture only. No requests run until explicitly started.");
        layout.addView(result);
        setContentView(layout);
        consumeIntent(getIntent());
    }

    @Override public void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        consumeIntent(intent);
    }

    private void consumeIntent(Intent intent) {
        if (intent != null && intent.getBooleanExtra("run", false)) {
            request(intent.getStringExtra("client"), intent.getStringExtra("url"));
        }
    }

    private static URL localUrl(String text) throws Exception {
        if (text == null || text.length() > 256) throw new IllegalArgumentException("Invalid fixture URL");
        URI uri = new URI(text);
        if (!("http".equals(uri.getScheme()) || "https".equals(uri.getScheme()))
                || !"127.0.0.1".equals(uri.getHost()) || uri.getPort() < 1 || uri.getPort() > 65535
                || uri.getRawUserInfo() != null || uri.getRawQuery() != null || uri.getRawFragment() != null
                || !(uri.getRawPath().isEmpty() || "/".equals(uri.getRawPath()))) {
            throw new IllegalArgumentException("Use http[s]://127.0.0.1:PORT/ without credentials or query");
        }
        return uri.toURL();
    }

    private void request(final String client, String text) {
        final URL url;
        try {
            if (!("platform".equals(client) || "okhttp".equals(client) || "okhttp-scope".equals(client))) throw new IllegalArgumentException("Unknown fixture client");
            url = localUrl(text);
        } catch (Exception invalid) {
            // URI parser messages can echo rejected input; never log those values.
            receipt(0, client, "invalid-input", 0, new IllegalArgumentException("Invalid local fixture input"));
            return;
        }
        if (!busy.compareAndSet(false, true)) {
            receipt(0, client, "busy", 0, null);
            return;
        }
        final int id = sequence.incrementAndGet();
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    int status = "platform".equals(client) ? platform(url) : okhttp(url, "okhttp-scope".equals(client));
                    receipt(id, client, "response", status, null);
                } catch (Exception error) {
                    // Preserve the request's error; never retry or convert rejection to success.
                    receipt(id, client, "error", 0, error);
                } finally { busy.set(false); }
            }
        }, "scope-fixture-request").start();
    }

    private static int platform(URL url) throws Exception {
        HttpURLConnection connection = (HttpURLConnection) url.openConnection();
        connection.setConnectTimeout(4000);
        connection.setReadTimeout(4000);
        connection.setInstanceFollowRedirects(false);
        connection.setRequestMethod("GET");
        if (connection instanceof HttpsURLConnection) {
            SSLContext context = SSLContext.getInstance("TLS");
            context.init(null, null, null); // Fresh default manager; Scope hooks this exact seam.
            ((HttpsURLConnection) connection).setSSLSocketFactory(context.getSocketFactory());
        }
        try { return connection.getResponseCode(); }
        finally { connection.disconnect(); }
    }

    private int okhttp(URL url, boolean fixtureTrust) throws Exception {
        TrustManagerFactory factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
        KeyStore anchors = null;
        if (fixtureTrust) {
            anchors = KeyStore.getInstance(KeyStore.getDefaultType());
            anchors.load(null, null);
            try (InputStream pem = getAssets().open("scope-ca.pem")) {
                anchors.setCertificateEntry("fixture-scope", CertificateFactory.getInstance("X.509").generateCertificate(pem));
            }
        }
        factory.init(anchors);
        X509TrustManager manager = null;
        for (TrustManager candidate : factory.getTrustManagers()) {
            if (candidate instanceof X509TrustManager) { manager = (X509TrustManager) candidate; break; }
        }
        if (manager == null) throw new IllegalStateException("No default X509 manager");
        SSLContext context = SSLContext.getInstance("TLS");
        context.init(null, new TrustManager[] {manager}, null);
        CertificatePinner pinner = new CertificatePinner.Builder()
                .add("127.0.0.1", "sha256/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").build();
        OkHttpClient client = new OkHttpClient.Builder()
                .sslSocketFactory(context.getSocketFactory(), manager).certificatePinner(pinner)
                .followRedirects(false).followSslRedirects(false).retryOnConnectionFailure(false)
                .connectTimeout(4, TimeUnit.SECONDS).readTimeout(4, TimeUnit.SECONDS)
                .writeTimeout(4, TimeUnit.SECONDS).callTimeout(8, TimeUnit.SECONDS).build();
        try (Response response = client.newCall(new Request.Builder().url(url).get().build()).execute()) {
            return response.code();
        } finally {
            client.connectionPool().evictAll();
            client.dispatcher().executorService().shutdownNow();
        }
    }

    private void receipt(int id, String client, String outcome, int status, Exception error) {
        try {
            JSONObject json = new JSONObject();
            json.put("fixture", "scope-engine-v1");
            json.put("pid", android.os.Process.myPid());
            json.put("request", id);
            json.put("client", "okhttp-scope".equals(client) ? "okhttp-scope" : ("okhttp".equals(client) ? "okhttp" : "platform"));
            json.put("outcome", outcome);
            json.put("status", status);
            if (error != null) {
                json.put("errorClass", error.getClass().getName());
                String message = error.getMessage();
                json.put("error", message == null ? "" : message.substring(0, Math.min(message.length(), 512)));
                if (error.getCause() != null) json.put("causeClass", error.getCause().getClass().getName());
            }
            final String line = json.toString();
            Log.i("ScopeEngineFixture", line);
            runOnUiThread(new Runnable() { @Override public void run() { result.setText(line); } });
        } catch (Exception encoding) { Log.e("ScopeEngineFixture", "receipt encoding failed"); }
    }
}
