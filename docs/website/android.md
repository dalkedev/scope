# Inspect an Android test app

Android interception is experimental. Begin with a device and application you control, and keep the device connected through cleanup. Ordinary desktop capture does not need Android, Python or Frida.

## Check the connection first

Install Android SDK Platform Tools, enable USB debugging and authorize your computer on the device. Confirm it is visible:

```sh
adb devices -l
```

In Snurpo, select a capture session, start capture, refresh the device list and select the device and installed package. Use **Diagnostics** to inspect authorization, root, ABI, runtime versions, mappings and recovery state.

Diagnostics is read-only. A ready preflight does not establish that a particular application or networking stack is compatible.

## Choose the interception method

| Mode | Behavior |
| --- | --- |
| **System proxy** | Sets the device-wide HTTP proxy and ADB reverse mapping, then restarts the selected app. Other apps can also appear in the capture. |
| **Frida** | Spawns and instruments the selected app process using the supported Java hooks. Requires a separately installed, matching worker/server runtime. |

System proxy mode does not install a system CA or bypass certificate pinning. HTTPS needs application trust configured separately; many Android apps ignore user-added CAs.

Frida's supported seams cover selected Java URL, SSLContext and OkHttp3 behavior. Native TLS, QUIC/HTTP3, child processes, custom pinners and lazy Kotlin pinners are outside the supported coverage.

## Optional Frida installation

Public packages exclude the native Frida worker/server runtime. The app reports an installation command with its actual paths. For the macOS app installed in Applications and an ARM64 Android device:

```sh
python3 /Applications/Snurpo.app/Contents/Resources/resources/device/install-runtime.py \
  --abi arm64-v8a
```

Use the ABI reported by `adb -s YOUR_SERIAL shell getprop ro.product.cpu.abi`. The helper requires Python 3.9+ with venv support, installs the pinned runtime locally and verifies the downloaded server hash. It does not start interception or change the device. Read the [full Android setup](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/ANDROID.md) for other hosts and overrides.

## Stop and restore

Press **Stop** while the device is connected. In Frida mode, Stop ends the exact instrumented app process and restores recorded Snurpo-owned state. System proxy mode restores recorded proxy and reverse mappings. External changes are preserved.

If cleanup fails, reconnect the same device and retry Stop. Keep `device-recovery.json`; deleting it does not restore settings. New interception is refused while cleanup remains unresolved.

Recorded acceptance covers one AOSP Android 16/API 36 ARM64 setup with Frida 17.9.1 and OkHttp 3.14.9. Other OS/ABI combinations, physical devices and active-worker crash/restart recovery remain unverified. [Detailed acceptance evidence](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/ANDROID_ACCEPTANCE.md).
