package com.scope.demo;

import android.app.Activity;
import android.os.Bundle;
import android.graphics.Color;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.view.View;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;

/** A deliberately small, local-only Android target for reproducible captures. */
public class MainActivity extends Activity {
    private TextView output;
    private final String base = "http://127.0.0.1:17891";

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setPadding(36, 48, 36, 24);
        layout.setBackgroundColor(Color.rgb(244, 247, 249));
        TextView title = new TextView(this);
        title.setText("SCOPE / ANDROID LAB");
        title.setTextSize(25);
        title.setTextColor(Color.rgb(24, 48, 61));
        layout.addView(title);
        TextView intro = new TextView(this);
        intro.setText("A real Android HTTP client. Requests go to your Mac's local Snurpo demo server through ADB.\n\nEdit a response in Snurpo, then see the result here.\n");
        intro.setTextSize(16);
        layout.addView(intro);
        addButton(layout, "GET /api/products", "GET", "/api/products", null);
        addButton(layout, "POST /api/login", "POST", "/api/login", "{\"email\":\"demo@scope.local\",\"password\":\"local-demo-only\"}");
        addButton(layout, "GET /api/orders", "GET", "/api/orders", null);
        addButton(layout, "GET /api/slow", "GET", "/api/slow", null);
        addButton(layout, "GET /api/error", "GET", "/api/error", null);
        output = new TextView(this);
        output.setTextSize(14);
        output.setTextColor(Color.rgb(30, 62, 75));
        output.setText("Ready. Start Snurpo interception for com.scope.demo, then tap a request.\n");
        ScrollView scroll = new ScrollView(this);
        scroll.addView(output);
        layout.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
        setContentView(layout);
    }

    private void addButton(LinearLayout layout, String label, final String method, final String path, final String body) {
        Button button = new Button(this);
        button.setText(label);
        button.setAllCaps(false);
        button.setOnClickListener(new View.OnClickListener() {
            public void onClick(View view) {
                new Thread(new Runnable() { public void run() { request(method, path, body); } }).start();
            }
        });
        layout.addView(button);
    }

    private void request(String method, String path, String body) {
        show("→ " + method + " " + path + "\n");
        HttpURLConnection connection = null;
        try {
            connection = (HttpURLConnection) new URL(base + path).openConnection();
            connection.setRequestMethod(method);
            connection.setConnectTimeout(10000);
            connection.setReadTimeout(120000); // Leave time for an interactive breakpoint.
            connection.setRequestProperty("X-Scope-Source", "android-demo");
            connection.setRequestProperty("Accept", "application/json");
            if (body != null) {
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "application/json");
                connection.getOutputStream().write(body.getBytes("UTF-8"));
                connection.getOutputStream().close();
            }
            int status = connection.getResponseCode();
            InputStream stream = status >= 400 ? connection.getErrorStream() : connection.getInputStream();
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            if (stream != null) {
                byte[] chunk = new byte[4096];
                int length;
                while ((length = stream.read(chunk)) != -1 && bytes.size() < 262144) bytes.write(chunk, 0, length);
                stream.close();
            }
            show("← " + status + " " + path + "\n" + bytes.toString("UTF-8") + "\n\n");
        } catch (Exception error) {
            show("ERROR: " + error.getMessage() + "\nCheck Snurpo proxy and adb reverse tcp:17891 tcp:17891.\n\n");
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private void show(final String message) { runOnUiThread(new Runnable() { public void run() { output.append(message); } }); }
}
