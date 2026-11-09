//! Read-only Android connection diagnostics. This module never downloads a runtime,
//! starts a server/app, attaches a script, or changes proxy/forwarding settings.
use crate::{adb, frida_arch, resource_file, Device, DeviceManager, Recovery};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    future::Future,
    io::Read,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub status: String,
    pub title: String,
    pub detail: String,
    pub action: Option<String>,
    pub command: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdbDiagnostics {
    pub path: String,
    pub available: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDiagnostics {
    pub serial: String,
    pub model: String,
    pub state: String,
    pub authorized: bool,
    pub root_status: String,
    pub root_method: Option<String>,
    pub abi: Option<String>,
    pub android_version: Option<String>,
    pub sdk: Option<u32>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FridaDiagnostics {
    pub runtime_kind: String,
    pub runtime_path: String,
    pub runtime_available: bool,
    pub runtime_version: Option<String>,
    pub runtime_error: Option<String>,
    pub server_source: String,
    pub server_path: Option<String>,
    pub server_available: bool,
    pub server_version: Option<String>,
    pub server_version_verified: bool,
    pub server_running: Option<bool>,
    pub version_match: Option<bool>,
    pub abi_supported: Option<bool>,
    pub server_error: Option<String>,
    pub experimental: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdbMapping {
    pub serial: String,
    pub kind: String,
    pub local: String,
    pub remote: String,
    pub owned: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MappingDiagnostics {
    pub reverse: Vec<AdbMapping>,
    pub forward: Vec<AdbMapping>,
    pub owned: Vec<AdbMapping>,
    pub global_proxy: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryDiagnostics {
    pub exists: bool,
    pub valid: bool,
    pub needs_cleanup: bool,
    pub active: bool,
    pub serial: Option<String>,
    pub package: Option<String>,
    pub mode: Option<String>,
    pub proxy_port: Option<u16>,
    pub forward_port: Option<u16>,
    pub server_pid: Option<u32>,
    pub ca_hash: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDiagnostics {
    pub checked_at: i64,
    pub read_only: bool,
    pub selected_serial: Option<String>,
    pub ready_for_proxy: bool,
    pub ready_for_frida: bool,
    pub adb: AdbDiagnostics,
    pub devices: Vec<Device>,
    pub device: Option<AndroidDiagnostics>,
    pub frida: FridaDiagnostics,
    pub mappings: MappingDiagnostics,
    pub recovery: RecoveryDiagnostics,
    pub checks: Vec<DiagnosticCheck>,
    pub notes: Vec<String>,
}

type ProbeFuture<'a> = Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
#[derive(Clone)]
struct RuntimeSelection {
    kind: String,
    path: PathBuf,
    binary: bool,
}
#[derive(Clone)]
pub(super) struct ServerSelection {
    pub(super) source: String,
    pub(super) path: Option<PathBuf>,
    pub(super) local: bool,
}
trait Probe: Send + Sync {
    fn adb<'a>(&'a self, serial: Option<&'a str>, arguments: &'a [&'a str]) -> ProbeFuture<'a>;
    fn runtime(&self) -> RuntimeSelection;
    fn worker_version(&self) -> ProbeFuture<'_>;
    fn server(&self, abi: Option<&str>) -> ServerSelection;
}
struct RealProbe<'a> {
    manager: &'a DeviceManager,
}
impl Probe for RealProbe<'_> {
    fn adb<'a>(&'a self, serial: Option<&'a str>, arguments: &'a [&'a str]) -> ProbeFuture<'a> {
        Box::pin(adb::run_timeout(
            &self.manager.inner.adb,
            serial,
            arguments,
            Duration::from_secs(4),
        ))
    }
    fn runtime(&self) -> RuntimeSelection {
        runtime_selection(&self.manager.inner.runtime)
    }
    fn worker_version(&self) -> ProbeFuture<'_> {
        let runtime = self.runtime();
        Box::pin(async move { probe_runtime(&runtime).await })
    }
    fn server(&self, abi: Option<&str>) -> ServerSelection {
        server_selection(&self.manager.inner.runtime, abi)
    }
}

impl DeviceManager {
    /// Inspect a connection without starting, stopping, installing or attaching anything.
    /// Missing tools/devices are returned as structured checks. Invalid serials are errors.
    pub async fn diagnostics(&self, serial: Option<String>) -> Result<DeviceDiagnostics> {
        self.collect_diagnostics(serial, &RealProbe { manager: self })
            .await
    }

    async fn collect_diagnostics(
        &self,
        serial: Option<String>,
        probe: &dyn Probe,
    ) -> Result<DeviceDiagnostics> {
        if let Some(serial) = &serial {
            adb::validate_serial(serial)?;
        }
        let active = self
            .inner
            .active
            .lock()
            .await
            .as_ref()
            .map(|v| v.recovery.clone());
        let (recovery, saved) = recovery_info(&self.recovery_path(), active.as_ref());
        let runtime = probe.runtime();
        let (adb_version, runtime_version) =
            tokio::join!(probe.adb(None, &["version"]), probe.worker_version());
        let mut result = DeviceDiagnostics {
            checked_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(i64::MAX as u128) as i64,
            read_only: true, selected_serial: serial.clone(), ready_for_proxy: false, ready_for_frida: false,
            adb: AdbDiagnostics {
                path: self.inner.adb.to_string_lossy().into_owned(), available: adb_version.is_ok(),
                version: adb_version.as_ref().ok().cloned(), error: adb_version.err().map(|e| format!("{e:#}")),
            },
            devices: vec![], device: None, mappings: MappingDiagnostics::default(), recovery,
            frida: FridaDiagnostics {
                runtime_kind: runtime.kind, runtime_path: runtime.path.to_string_lossy().into_owned(),
                runtime_available: runtime_version.is_ok(), runtime_version: runtime_version.as_ref().ok().cloned(),
                runtime_error: runtime_version.err().map(|e| format!("{e:#}")), server_source: "notChecked".into(),
                server_path: None, server_available: false, server_version: None, server_version_verified: false,
                server_running: None, version_match: None, abi_supported: None, server_error: None, experimental: true,
            },
            checks: vec![], notes: vec![
                "Read-only: no app/server is started, no script is attached, no runtime is downloaded, and no proxy/certificate/ADB mapping is changed.".into(),
                "Only the selected device's root access and properties are checked. Other devices' root flags are not probed.".into(),
                "Readiness means basic connection preflight; it does not prove interception coverage or universal Android/app compatibility.".into(),
                "Root probing runs id through an existing su binary and can display that device's superuser authorization prompt; each ADB probe is bounded.".into(),
            ],
        };
        if result.adb.available {
            match probe.adb(None, &["devices", "-l"]).await {
                Ok(text) => result.devices = adb::parse_devices(&text),
                Err(error) => result.check(
                    "adb.devices",
                    "error",
                    "Device enumeration failed",
                    &format!("{error:#}"),
                    Some("Check the ADB installation/USB connection and refresh diagnostics."),
                    None,
                ),
            }
        }
        if result.selected_serial.is_none() && result.devices.len() == 1 {
            result.selected_serial = Some(result.devices[0].serial.clone());
        }
        let selected = result
            .selected_serial
            .as_ref()
            .and_then(|serial| result.devices.iter().find(|d| &d.serial == serial))
            .cloned();
        if let Some(device) = selected {
            let mut info = AndroidDiagnostics {
                serial: device.serial.clone(),
                model: device.model,
                state: device.state.clone(),
                authorized: device.state == "device",
                root_status: "unknown".into(),
                root_method: None,
                abi: None,
                android_version: None,
                sdk: None,
                error: None,
            };
            if info.authorized {
                let selected_serial = info.serial.as_str();
                let (identity, abi, release, sdk, proxy, reverse, forward, processes) = tokio::join!(
                    probe.adb(Some(selected_serial), &["shell", "id"]),
                    probe.adb(
                        Some(selected_serial),
                        &["shell", "getprop", "ro.product.cpu.abi"]
                    ),
                    probe.adb(
                        Some(selected_serial),
                        &["shell", "getprop", "ro.build.version.release"]
                    ),
                    probe.adb(
                        Some(selected_serial),
                        &["shell", "getprop", "ro.build.version.sdk"]
                    ),
                    probe.adb(
                        Some(selected_serial),
                        &["shell", "settings", "get", "global", "http_proxy"]
                    ),
                    probe.adb(Some(selected_serial), &["reverse", "--list"]),
                    probe.adb(Some(selected_serial), &["forward", "--list"]),
                    probe.adb(
                        Some(selected_serial),
                        &["shell", "ps", "-A", "-o", "PID,NAME,ARGS"]
                    ),
                );
                info.abi = nonempty(abi.ok());
                info.android_version = nonempty(release.ok());
                info.sdk = sdk.ok().and_then(|value| value.trim().parse().ok());
                match identity {
                    Ok(identity) if is_root(&identity) => {
                        info.root_status = "rooted".into();
                        info.root_method = Some("adbShell".into());
                    }
                    Ok(_) => match probe
                        .adb(Some(selected_serial), &["shell", "su", "-c", "'id'"])
                        .await
                    {
                        Ok(identity) if is_root(&identity) => {
                            info.root_status = "rooted".into();
                            info.root_method = Some("su".into());
                        }
                        Ok(_) => info.root_status = "rootless".into(),
                        Err(error) => {
                            let text = format!("{error:#}");
                            info.root_status = if text.contains("timed out") {
                                "unknown"
                            } else {
                                "rootless"
                            }
                            .into();
                            info.error = Some(text);
                        }
                    },
                    Err(error) => info.error = Some(format!("ADB shell is unavailable: {error:#}")),
                }
                if let Some(device) = result.devices.iter_mut().find(|d| d.serial == info.serial) {
                    device.rooted = info.root_status == "rooted";
                }
                let owner = active
                    .as_ref()
                    .or(saved.as_ref())
                    .filter(|r| r.serial == info.serial);
                result.mappings.global_proxy = nonempty(proxy.ok()).filter(|s| s != "null");
                let mut mapping_errors = vec![];
                match reverse {
                    Ok(text) => {
                        result.mappings.reverse =
                            parse_mappings(&text, "reverse", &info.serial, owner)
                    }
                    Err(error) => mapping_errors.push(format!("Reverse mappings: {error:#}")),
                }
                match forward {
                    Ok(text) => {
                        result.mappings.forward =
                            parse_mappings(&text, "forward", &info.serial, owner)
                    }
                    Err(error) => mapping_errors.push(format!("Forward mappings: {error:#}")),
                }
                result.mappings.owned = result
                    .mappings
                    .reverse
                    .iter()
                    .chain(result.mappings.forward.iter())
                    .filter(|m| m.owned)
                    .cloned()
                    .collect();
                result.mappings.error =
                    (!mapping_errors.is_empty()).then(|| mapping_errors.join("; "));
                result.frida.abi_supported =
                    info.abi.as_deref().map(|abi| frida_arch(abi).is_some());
                let processes = processes.ok();
                result.frida.server_running = processes.as_ref().map(|text| {
                    text.lines()
                        .any(|line| line.contains("frida-server") || line.contains("adirf-server"))
                });
                let server = probe.server(info.abi.as_deref());
                result.frida.server_source = server.source.clone();
                result.frida.server_path = server
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned());
                if let Some(path) = server.path {
                    if server.local {
                        match inspect_server(&path, info.abi.as_deref()) {
                            Ok(()) => result.frida.server_available = true,
                            Err(error) => result.frida.server_error = Some(format!("{error:#}")),
                        }
                    } else if valid_device_path(&path) {
                        let path = path.to_string_lossy();
                        let executable = format!("test -x {}", adb::quote(&path));
                        if probe
                            .adb(Some(selected_serial), &["shell", &executable])
                            .await
                            .is_ok()
                        {
                            result.frida.server_available = true;
                            let version_command = format!("{} --version", adb::quote(&path));
                            match probe
                                .adb(Some(selected_serial), &["shell", &version_command])
                                .await
                            {
                                Ok(version) => {
                                    result.frida.server_version = nonempty(Some(version));
                                    result.frida.server_version_verified =
                                        result.frida.server_version.is_some();
                                    if !result.frida.server_version_verified {
                                        result.frida.server_error = Some(
                                            "Server --version returned an empty version".into(),
                                        );
                                    }
                                }
                                Err(error) => {
                                    result.frida.server_error =
                                        Some(format!("Server --version failed: {error:#}"))
                                }
                            }
                        } else {
                            result.frida.server_error =
                                Some(format!("No executable server found at {path}"));
                        }
                    } else {
                        result.frida.server_error = Some("Configured device server path is invalid; use an absolute path without shell characters.".into());
                    }
                }
                if result.frida.server_version_verified {
                    result.frida.version_match = result
                        .frida
                        .runtime_version
                        .as_ref()
                        .zip(result.frida.server_version.as_ref())
                        .map(|(left, right)| left == right);
                }
            }
            result.device = Some(info);
        }
        result.add_checks(self);
        let connected = result.device.as_ref().is_some_and(|d| {
            d.authorized
                && !d
                    .error
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("ADB shell is unavailable")
        });
        let session_free = !result.recovery.exists && !result.recovery.active;
        result.ready_for_proxy = result.adb.available && connected && session_free;
        let server_can_start = result.frida.server_source != "existingUsb"
            || result.frida.server_running == Some(true);
        result.ready_for_frida = result.ready_for_proxy
            && result
                .device
                .as_ref()
                .is_some_and(|d| d.root_status == "rooted")
            && result.frida.runtime_available
            && result.frida.abi_supported == Some(true)
            && result.frida.server_available
            && result.frida.server_error.is_none()
            && server_can_start
            && result.frida.version_match != Some(false);
        Ok(result)
    }
}

impl DeviceDiagnostics {
    fn check(
        &mut self,
        id: &str,
        status: &str,
        title: &str,
        detail: &str,
        action: Option<&str>,
        command: Option<String>,
    ) {
        self.checks.push(DiagnosticCheck {
            id: id.into(),
            status: status.into(),
            title: title.into(),
            detail: detail.into(),
            action: action.map(str::to_owned),
            command,
        });
    }
    fn add_checks(&mut self, manager: &DeviceManager) {
        if self.adb.available {
            self.check(
                "adb.available",
                "ok",
                "ADB is available",
                &self.adb.version.clone().unwrap_or_default(),
                None,
                None,
            );
        } else {
            self.check("adb.available", "error", "ADB is unavailable", &self.adb.error.clone().unwrap_or_default(),
                Some("Install Android SDK platform-tools, set ANDROID_HOME/ANDROID_SDK_ROOT, or point SCOPE_ADB to the executable."), None);
        }
        if !self.recovery.valid {
            self.check("recovery.state", "error", "Recovery file cannot be trusted", &self.recovery.error.clone().unwrap_or_default(),
                Some("Inspect the saved recovery file and reconnect the recorded device. Do not erase it as a substitute for restoring device settings."), None);
        } else if self.recovery.needs_cleanup {
            let detail = format!(
                "A previous session for {} needs cleanup before another start.",
                self.recovery
                    .serial
                    .as_deref()
                    .unwrap_or("the recorded device")
            );
            self.check(
                "recovery.state",
                "warning",
                "Device cleanup is pending",
                &detail,
                Some("Reconnect that same device and use Stop / restore settings."),
                None,
            );
        } else if self.recovery.active {
            self.check("recovery.state", "info", "An owned device session is active", "Its recorded mappings remain owned by this session. Stop it before starting another session.", None, None);
        } else {
            self.check(
                "recovery.state",
                "ok",
                "No pending device cleanup",
                "No saved or active device interception state was found.",
                None,
                None,
            );
        }
        if let Some(device) = self.device.clone() {
            if device.authorized {
                self.check(
                    "device.connection",
                    "ok",
                    "Device is authorized",
                    &format!("{} ({})", device.model, device.serial),
                    None,
                    None,
                );
                if let Some(error) = device
                    .error
                    .as_deref()
                    .filter(|error| error.starts_with("ADB shell is unavailable"))
                {
                    self.check("device.shell", "error", "ADB shell access failed", error,
                        Some("Unlock/reconnect the selected device and check its debugging authorization before starting interception."), None);
                }
                match device.root_status.as_str() {
                    "rooted" => self.check("device.root", "ok", "Root access confirmed", &format!("Read-only id probe succeeded via {}.", device.root_method.as_deref().unwrap_or("shell")), None, None),
                    "rootless" => self.check("device.root", "warning", "Root access was not granted", "Proxy mode can capture HTTP; this Frida integration and temporary system CA injection require root.",
                        Some("Use an authorized rooted lab device, grant Shell superuser access, or choose system proxy mode with the app's own CA trust configuration."), None),
                    _ => self.check("device.root", "warning", "Root access is unknown", device.error.as_deref().unwrap_or("The root probe did not complete."),
                        Some("Check the device's superuser prompt and rerun diagnostics. Diagnostics never roots a device."), None),
                }
                match self.frida.abi_supported {
                    Some(true) => self.check("device.abi", "ok", "Android ABI recognized", device.abi.as_deref().unwrap_or(""), None, None),
                    Some(false) => self.check("device.abi", "error", "Android ABI has no supported server mapping", device.abi.as_deref().unwrap_or(""),
                        Some("Use arm64-v8a, armeabi-v7a, x86 or x86_64 for Frida mode; system proxy mode does not require a Frida ABI."), None),
                    None => self.check("device.abi", "warning", "Android ABI is unknown", "getprop did not return an ABI.", Some("Check ADB shell access and rerun diagnostics."), None),
                }
                let detail = format!("Android {} / API {}. Only a short Android 16 arm64 demo smoke was verified; native TLS, QUIC, child processes and app-specific defenses are not guaranteed.",
                    device.android_version.as_deref().unwrap_or("unknown"), device.sdk.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()));
                self.check("frida.compatibility", "warning", "Frida Java hooks remain experimental", &detail,
                    Some("Use an app/device you are authorized to test. Android 16 ART/GC crashes were observed upstream; saved captures remain independent of the worker."), None);
                if let Some(error) = self.mappings.error.clone() {
                    self.check(
                        "adb.mappings",
                        "warning",
                        "Mapping inspection was incomplete",
                        &error,
                        Some("Check ADB connectivity before cleanup/start."),
                        None,
                    );
                } else {
                    self.check(
                        "adb.mappings",
                        "info",
                        "Current ADB mappings inspected",
                        &format!(
                            "{} reverse, {} forward; {} match Scope's recorded ownership.",
                            self.mappings.reverse.len(),
                            self.mappings.forward.len(),
                            self.mappings.owned.len()
                        ),
                        None,
                        None,
                    );
                }
                if let Some(proxy) = self.mappings.global_proxy.clone() {
                    self.check(
                        "device.proxy",
                        "info",
                        "Android global proxy",
                        &format!(
                            "{proxy}. This setting is device-wide; diagnostics does not alter it."
                        ),
                        None,
                        None,
                    );
                }
            } else {
                let (title, action) = match device.state.as_str() {
                    "unauthorized" => ("USB debugging authorization is required", "Unlock the device and accept this computer's USB debugging prompt, then refresh."),
                    "offline" => ("Device is offline", "Reconnect USB or the existing network debugging connection. Diagnostics will not reboot or reconnect it automatically."),
                    "no permissions" => ("Host has no USB permission", "Configure Android udev access on Linux or select an authorized debugging connection."),
                    _ => ("Device is not ready", "Resolve the ADB device state, then refresh diagnostics."),
                };
                self.check(
                    "device.connection",
                    "error",
                    title,
                    &format!("{}: {}", device.serial, device.state),
                    Some(action),
                    None,
                );
            }
        } else if self.selected_serial.is_some() {
            self.check(
                "device.connection",
                "error",
                "Selected device is disconnected",
                "The requested serial is absent from adb devices.",
                Some("Reconnect that same device or choose a connected device."),
                None,
            );
        } else if self.devices.len() > 1 {
            self.check(
                "device.connection",
                "info",
                "Select a device",
                "Several ADB devices are connected; none was chosen automatically.",
                Some("Choose the exact device to inspect root, ABI, mappings and server state."),
                None,
            );
        } else {
            self.check("device.connection", "warning", "No Android device is connected", "No device-specific probes were attempted.", Some("Connect a USB-debugging device or manually start a lab emulator. Diagnostics never boots an emulator."), None);
        }
        let abi = self.device.as_ref().and_then(|d| d.abi.clone());
        if self.frida.runtime_available {
            self.check(
                "frida.runtime",
                "ok",
                "Optional Frida runtime is available",
                &format!(
                    "{} via {} ({})",
                    self.frida.runtime_version.as_deref().unwrap_or("unknown"),
                    self.frida.runtime_kind,
                    self.frida.runtime_path
                ),
                None,
                None,
            );
        } else {
            let command = abi
                .as_deref()
                .filter(|abi| frida_arch(abi).is_some())
                .map(|abi| manager.setup_command(abi));
            self.check("frida.runtime", "warning", "Optional Frida runtime needs setup", &self.frida.runtime_error.clone().unwrap_or_default(),
                Some("Run the explicit setup helper for the selected ABI, then refresh. It uses a private virtualenv; diagnostics never downloads it."), command);
        }
        if self.device.as_ref().is_some_and(|d| d.authorized) {
            self.check("frida.process", "info", "Observed Frida process state",
                match self.frida.server_running {
                    Some(true) => "A process name containing frida-server/adirf-server was observed. Its listener, permissions and running version were not verified by attachment.",
                    Some(false) => "No recognized Frida server process name was observed. A configured local/device binary can be launched later by Start.",
                    None => "The Android process list could not be read. Server process state is unknown.",
                }, None, None);
            if self.frida.server_available {
                let detail = format!(
                    "{}: {}. {}",
                    self.frida.server_source,
                    self.frida.server_path.as_deref().unwrap_or("unknown"),
                    if self.frida.server_version_verified {
                        "The device binary's --version was read without starting a listener."
                    } else {
                        "Local ELF architecture was checked. The version is not assumed from its filename; Start verifies it on the device before launching a listener."
                    }
                );
                self.check("frida.server", if self.frida.server_source == "existingUsb" && self.frida.server_running != Some(true) { "warning" } else { "ok" },
                    "Frida server source inspected", &detail,
                    (self.frida.server_source == "existingUsb" && self.frida.server_running != Some(true)).then_some("Install a managed server with the setup helper, configure an existing executable, or explicitly start your own USB server. Diagnostics does not start it."),
                    None);
            } else {
                self.check("frida.server", "warning", "No usable Frida server source was confirmed", &self.frida.server_error.clone().unwrap_or_else(||"Select a device with a supported ABI.".into()),
                    Some("Install the managed runtime for that ABI or configure SCOPE_FRIDA_SERVER_LOCAL / SCOPE_FRIDA_SERVER. An existing server at a nonstandard path/port may need explicit configuration."),
                    abi.as_deref().filter(|abi|frida_arch(abi).is_some()).map(|abi|manager.setup_command(abi)));
            }
            if self.frida.version_match == Some(false) {
                self.check("frida.versions", "error", "Frida client/server version mismatch",
                    &format!("Worker {} versus device binary {}.", self.frida.runtime_version.as_deref().unwrap_or("unknown"), self.frida.server_version.as_deref().unwrap_or("unknown")),
                    Some("Use the exact same Frida version for Python/worker and server. Remove conflicting overrides or rerun the managed setup helper."), None);
            } else if self.frida.version_match == Some(true) {
                self.check(
                    "frida.versions",
                    "ok",
                    "Frida versions match",
                    self.frida.runtime_version.clone().as_deref().unwrap_or(""),
                    None,
                    None,
                );
            } else {
                self.check("frida.versions", "info", "Server version is not yet verified", "Diagnostics does not upload a local Android executable to inspect it. Start performs the actual device --version check before starting its server.", None, None);
            }
        }
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
}
pub(super) fn is_root(identity: &str) -> bool {
    identity
        .split_whitespace()
        .any(|field| field == "uid=0(root)" || field == "uid=0")
}
fn runtime_selection(runtime: &Path) -> RuntimeSelection {
    if let Some(path) = env::var_os("SCOPE_FRIDA_WORKER") {
        return RuntimeSelection {
            kind: "explicitBinary".into(),
            path: path.into(),
            binary: true,
        };
    }
    if let Some(path) = env::var_os("SCOPE_PYTHON") {
        return RuntimeSelection {
            kind: "explicitPython".into(),
            path: path.into(),
            binary: false,
        };
    }
    if let Some(path) = resource_file(if cfg!(windows) {
        "bin/scope-frida-worker.exe"
    } else {
        "bin/scope-frida-worker"
    }) {
        return RuntimeSelection {
            kind: "bundled".into(),
            path,
            binary: true,
        };
    }
    let path = runtime.join("managed-frida/venv").join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    });
    if path.is_file() {
        return RuntimeSelection {
            kind: "managed".into(),
            path,
            binary: false,
        };
    }
    RuntimeSelection {
        kind: "systemPython".into(),
        path: PathBuf::from(if cfg!(windows) { "python" } else { "python3" }),
        binary: false,
    }
}
async fn probe_runtime(runtime: &RuntimeSelection) -> Result<String> {
    let mut command = Command::new(&runtime.path);
    if runtime.binary {
        command.arg("--version");
    } else {
        command.args([
            "-B",
            "-c",
            "import json,frida; print(json.dumps({'kind':'version','version':frida.__version__}))",
        ]);
    }
    command
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let output = tokio::time::timeout(Duration::from_secs(8), command.output())
        .await
        .map_err(|_| anyhow::anyhow!("Frida runtime version probe timed out after 8 seconds"))?
        .map_err(|e| anyhow::anyhow!("Cannot execute {}: {e}", runtime.path.display()))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            runtime.path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_owned))
        })
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Frida runtime did not report a version"))
}
pub(super) fn server_selection(runtime: &Path, abi: Option<&str>) -> ServerSelection {
    if let Some(path) = env::var_os("SCOPE_FRIDA_SERVER_LOCAL") {
        return ServerSelection {
            source: "localOverride".into(),
            path: Some(path.into()),
            local: true,
        };
    }
    if let Some(path) = env::var_os("SCOPE_FRIDA_SERVER") {
        return ServerSelection {
            source: "deviceOverride".into(),
            path: Some(path.into()),
            local: false,
        };
    }
    if let Some(arch) = abi.and_then(frida_arch) {
        let file = format!("frida-server-17.9.1-android-{arch}");
        if let Some(path) = resource_file(&format!("bin/{file}")) {
            return ServerSelection {
                source: "bundled".into(),
                path: Some(path),
                local: true,
            };
        }
        let path = runtime.join("managed-frida").join(file);
        if path.is_file() {
            return ServerSelection {
                source: "managed".into(),
                path: Some(path),
                local: true,
            };
        }
    }
    ServerSelection {
        source: "existingUsb".into(),
        path: Some("/data/local/tmp/frida-server".into()),
        local: false,
    }
}
pub(super) async fn runtime_version(runtime: &Path) -> Result<String> {
    probe_runtime(&runtime_selection(runtime)).await
}
pub(super) fn valid_device_path(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.starts_with('/')
        && value.len() > 1
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_./-".contains(&c))
}
/// Validate the target ELF architecture without running or copying the binary.
pub(super) fn inspect_server(path: &Path, abi: Option<&str>) -> Result<()> {
    let abi = abi.ok_or_else(|| {
        anyhow::anyhow!("Device ABI is unknown; cannot verify a local server's architecture")
    })?;
    let (expected, class) = match frida_arch(abi) {
        Some("arm64") => (183, 2),
        Some("arm") => (40, 1),
        Some("x86") => (3, 1),
        Some("x86_64") => (62, 2),
        _ => bail!("Unsupported Android ABI: {abi}"),
    };
    let mut header = [0_u8; 20];
    fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Cannot read Frida server {}: {e}", path.display()))?
        .read_exact(&mut header)
        .map_err(|e| anyhow::anyhow!("Invalid/truncated Android ELF server: {e}"))?;
    if &header[..4] != b"\x7fELF" || !matches!(header[4], 1 | 2) || !matches!(header[5], 1 | 2) {
        bail!("Configured server is not an Android ELF executable");
    }
    let actual = if header[5] == 1 {
        u16::from_le_bytes([header[18], header[19]])
    } else {
        u16::from_be_bytes([header[18], header[19]])
    };
    if actual != expected {
        bail!("Frida server ELF architecture {actual} does not match Android ABI {abi} (expected {expected})");
    }
    if header[4] != class || header[5] != 1 {
        bail!("Frida server ELF class/byte order does not match Android ABI {abi}");
    }
    Ok(())
}
fn recovery_info(
    path: &Path,
    active: Option<&Recovery>,
) -> (RecoveryDiagnostics, Option<Recovery>) {
    let exists = path.exists();
    let parsed = if exists {
        fs::read(path)
            .map_err(anyhow::Error::from)
            .and_then(|data| serde_json::from_slice::<Recovery>(&data).map_err(anyhow::Error::from))
            .and_then(|state| {
                validate_recovery(&state)?;
                Ok(state)
            })
    } else {
        Err(anyhow::anyhow!("No recovery file"))
    };
    let error = if exists {
        parsed.as_ref().err().map(|e| format!("{e:#}"))
    } else {
        None
    };
    let saved = parsed.ok();
    let state = active.or(saved.as_ref());
    (
        RecoveryDiagnostics {
            exists,
            valid: !exists || saved.is_some(),
            needs_cleanup: exists && active.is_none(),
            active: active.is_some(),
            serial: state.map(|s| s.serial.clone()),
            package: state.map(|s| s.package.clone()),
            mode: state.map(|s| s.mode.clone()),
            proxy_port: state.map(|s| s.port),
            forward_port: state.and_then(|s| s.forward_port),
            server_pid: state.and_then(|s| s.server_pid),
            ca_hash: state.and_then(|s| s.ca_hash.clone()),
            error,
        },
        saved,
    )
}
pub(super) fn validate_recovery(state: &Recovery) -> Result<()> {
    adb::validate_serial(&state.serial)?;
    adb::validate_package(&state.package)?;
    if state.port == 0
        || !["proxy", "frida"].contains(&state.mode.as_str())
        || state.server_pid.is_some_and(|pid| pid <= 1)
        || state.forward_port == Some(0)
        || state
            .ca_hash
            .as_ref()
            .is_some_and(|hash| hash.len() != 8 || !hash.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        bail!("Invalid saved recovery fields");
    }
    if state.previous_reverse.as_ref().is_some_and(|target| {
        target.is_empty()
            || !target
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b":/._-".contains(&c))
    }) {
        bail!("Invalid saved reverse mapping");
    }
    Ok(())
}
pub(super) fn parse_mappings(
    text: &str,
    kind: &str,
    serial: &str,
    owner: Option<&Recovery>,
) -> Vec<AdbMapping> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 3 || (kind == "forward" && fields[0] != serial) {
                return None;
            }
            let owned = owner.is_some_and(|state| {
                if kind == "reverse" {
                    fields[1] == format!("tcp:{}", state.port)
                        && fields[2] == format!("tcp:{}", state.port)
                } else {
                    state.forward_port.is_some_and(|port| {
                        fields[1] == format!("tcp:{port}") && fields[2] == "tcp:24073"
                    })
                }
            });
            Some(AdbMapping {
                serial: serial.into(),
                kind: kind.into(),
                local: fields[1].into(),
                remote: fields[2].into(),
                owned,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Fixture {
        runtime_version: String,
        responses: Vec<Response>,
    }
    #[derive(Deserialize)]
    struct Response {
        serial: Option<String>,
        args: Vec<String>,
        stdout: Option<String>,
        error: Option<String>,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Call {
        serial: Option<String>,
        args: Vec<String>,
    }
    struct FakeProbe {
        fixture: Fixture,
        calls: Mutex<Vec<Call>>,
        server: ServerSelection,
        runtime_error: Option<String>,
    }
    impl FakeProbe {
        fn android() -> Self {
            Self {
                fixture: serde_json::from_str(include_str!("../tests/fixtures/diagnostics.json"))
                    .unwrap(),
                calls: Mutex::new(vec![]),
                server: ServerSelection {
                    source: "existingUsb".into(),
                    path: Some("/data/local/tmp/frida-server".into()),
                    local: false,
                },
                runtime_error: None,
            }
        }
        fn set(&mut self, serial: Option<&str>, args: &[&str], result: Result<&str, &str>) {
            self.fixture.responses.retain(|response| {
                response.serial.as_deref() != serial
                    || response.args.iter().map(String::as_str).collect::<Vec<_>>() != args
            });
            self.fixture.responses.push(Response {
                serial: serial.map(str::to_owned),
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                stdout: result.as_ref().ok().map(|value| (*value).to_owned()),
                error: result.err().map(str::to_owned),
            });
        }
        fn assert_read_only(&self) {
            for call in self.calls.lock().unwrap().iter() {
                let args: Vec<_> = call.args.iter().map(String::as_str).collect();
                let allowed = matches!(
                    args.as_slice(),
                    ["version"]
                        | ["devices", "-l"]
                        | ["reverse", "--list"]
                        | ["forward", "--list"]
                        | ["shell", "id"]
                        | ["shell", "su", "-c", "'id'"]
                        | ["shell", "getprop", "ro.product.cpu.abi"]
                        | ["shell", "getprop", "ro.build.version.release"]
                        | ["shell", "getprop", "ro.build.version.sdk"]
                        | ["shell", "settings", "get", "global", "http_proxy"]
                        | ["shell", "ps", "-A", "-o", "PID,NAME,ARGS"]
                        | ["shell", "test -x '/data/local/tmp/frida-server'"]
                        | ["shell", "'/data/local/tmp/frida-server' --version"]
                );
                assert!(allowed, "Unexpected diagnostic command: {call:?}");
            }
        }
    }
    impl Probe for FakeProbe {
        fn adb<'a>(&'a self, serial: Option<&'a str>, args: &'a [&'a str]) -> ProbeFuture<'a> {
            self.calls.lock().unwrap().push(Call {
                serial: serial.map(str::to_owned),
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            });
            Box::pin(async move {
                let response = self.fixture.responses.iter().find(|response| {
                    response.serial.as_deref() == serial
                        && response.args.iter().map(String::as_str).collect::<Vec<_>>() == args
                });
                let response = response.ok_or_else(|| {
                    anyhow::anyhow!("Fake ADB has no response for {serial:?} {args:?}")
                })?;
                if let Some(error) = &response.error {
                    bail!("{error}");
                }
                Ok(response.stdout.clone().unwrap_or_default())
            })
        }
        fn runtime(&self) -> RuntimeSelection {
            RuntimeSelection {
                kind: "fixture".into(),
                path: "fixture-python".into(),
                binary: false,
            }
        }
        fn worker_version(&self) -> ProbeFuture<'_> {
            Box::pin(async {
                if let Some(error) = &self.runtime_error {
                    bail!("{error}");
                }
                Ok(self.fixture.runtime_version.clone())
            })
        }
        fn server(&self, _abi: Option<&str>) -> ServerSelection {
            self.server.clone()
        }
    }
    fn check<'a>(report: &'a DeviceDiagnostics, id: &str) -> &'a DiagnosticCheck {
        report.checks.iter().find(|check| check.id == id).unwrap()
    }
    fn saved_state() -> Recovery {
        Recovery {
            serial: "fixture-01".into(),
            package: "com.scope.demo".into(),
            mode: "frida".into(),
            port: 17890,
            previous_proxy: None,
            previous_reverse: None,
            forward_port: Some(42123),
            server_pid: Some(481),
            ca_hash: None,
            root_shell: true,
            target_spawn_pending: false,
            target_pid: None,
        }
    }
    fn elf(path: &Path, machine: u16, class: u8, endian: u8) {
        let mut bytes = [0_u8; 20];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = class;
        bytes[5] = endian;
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        fs::write(path, bytes).unwrap();
    }

    #[tokio::test]
    async fn diagnostics_ready_fixture_is_read_only_and_serializes_public_contract() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().join("untouched"));
        let probe = FakeProbe::android();
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.read_only && report.ready_for_proxy && report.ready_for_frida);
        assert_eq!(report.selected_serial.as_deref(), Some("fixture-01"));
        assert_eq!(report.device.as_ref().unwrap().root_status, "rooted");
        assert_eq!(report.frida.version_match, Some(true));
        assert!(report.frida.experimental);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["readOnly"], true);
        assert_eq!(json["frida"]["serverVersionVerified"], true);
        assert!(json.get("ready_for_frida").is_none());
        assert!(
            !manager.inner.runtime.exists(),
            "Diagnostics created host state"
        );
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn missing_adb_and_runtime_are_structured_failures_without_device_queries() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let mut probe = FakeProbe::android();
        probe.set(
            None,
            &["version"],
            Err("Cannot execute adb: executable not found"),
        );
        probe.runtime_error = Some("No module named frida".into());
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(!report.adb.available && !report.ready_for_proxy && !report.ready_for_frida);
        assert_eq!(check(&report, "adb.available").status, "error");
        assert!(check(&report, "adb.available")
            .action
            .as_ref()
            .unwrap()
            .contains("platform-tools"));
        assert_eq!(probe.calls.lock().unwrap().len(), 1);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn unready_devices_never_run_shell_probes_and_offer_specific_fixes() {
        for (state, expected) in [
            ("unauthorized", "accept this computer"),
            ("offline", "Reconnect USB"),
            ("no permissions", "udev"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(dir.path().into());
            let mut probe = FakeProbe::android();
            probe.set(
                None,
                &["devices", "-l"],
                Ok(&format!("fixture-01 {state}\n")),
            );
            let report = manager.collect_diagnostics(None, &probe).await.unwrap();
            assert!(!report.ready_for_proxy && !report.ready_for_frida);
            assert_eq!(report.device.as_ref().unwrap().state, state);
            assert!(check(&report, "device.connection")
                .action
                .as_ref()
                .unwrap()
                .contains(expected));
            assert_eq!(probe.calls.lock().unwrap().len(), 2);
            probe.assert_read_only();
        }
    }

    #[tokio::test]
    async fn multiple_devices_require_selection_and_disconnected_selection_is_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let mut probe = FakeProbe::android();
        probe.set(
            None,
            &["devices", "-l"],
            Ok("fixture-01 device\nfixture-02 device\n"),
        );
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.selected_serial.is_none() && report.device.is_none());
        assert_eq!(check(&report, "device.connection").title, "Select a device");
        assert_eq!(probe.calls.lock().unwrap().len(), 2);
        let report = manager
            .collect_diagnostics(Some("missing".into()), &probe)
            .await
            .unwrap();
        assert_eq!(
            check(&report, "device.connection").title,
            "Selected device is disconnected"
        );
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn root_denial_timeout_and_shell_failure_have_different_meanings() {
        for (failure, root_status, proxy) in [
            ("su: permission denied", "rootless", true),
            ("ADB command timed out after 4 seconds", "unknown", true),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(dir.path().into());
            let mut probe = FakeProbe::android();
            probe.set(
                Some("fixture-01"),
                &["shell", "id"],
                Ok("uid=2000(shell) gid=2000(shell)"),
            );
            probe.set(
                Some("fixture-01"),
                &["shell", "su", "-c", "'id'"],
                Err(failure),
            );
            let report = manager.collect_diagnostics(None, &probe).await.unwrap();
            assert_eq!(report.device.unwrap().root_status, root_status);
            assert_eq!(report.ready_for_proxy, proxy);
            assert!(!report.ready_for_frida);
            probe.assert_read_only();
        }
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let mut probe = FakeProbe::android();
        probe.set(
            Some("fixture-01"),
            &["shell", "id"],
            Err("device disconnected"),
        );
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(!report.ready_for_proxy && !report.ready_for_frida);
        assert_eq!(report.device.unwrap().root_status, "unknown");
    }

    #[tokio::test]
    async fn unsupported_abi_and_verified_version_mismatch_block_frida_only() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let mut probe = FakeProbe::android();
        probe.set(
            Some("fixture-01"),
            &["shell", "getprop", "ro.product.cpu.abi"],
            Ok("riscv64"),
        );
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.ready_for_proxy && !report.ready_for_frida);
        assert_eq!(check(&report, "device.abi").status, "error");
        probe.set(
            Some("fixture-01"),
            &["shell", "getprop", "ro.product.cpu.abi"],
            Ok("arm64-v8a"),
        );
        probe.set(
            Some("fixture-01"),
            &["shell", "'/data/local/tmp/frida-server' --version"],
            Ok("16.0.0"),
        );
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.ready_for_proxy && !report.ready_for_frida);
        assert_eq!(report.frida.version_match, Some(false));
        assert_eq!(check(&report, "frida.versions").status, "error");
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn local_elf_checks_architecture_without_uploading_or_guessing_version() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("frida-server-17.9.1-android-arm64");
        let manager = DeviceManager::new(dir.path().join("runtime"));
        let mut probe = FakeProbe::android();
        probe.server = ServerSelection {
            source: "managed".into(),
            path: Some(binary.clone()),
            local: true,
        };
        elf(&binary, 62, 2, 1);
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(!report.ready_for_frida && !report.frida.server_available);
        assert!(report
            .frida
            .server_error
            .unwrap()
            .contains("does not match"));
        elf(&binary, 183, 2, 1);
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.ready_for_frida && report.frida.server_available);
        assert!(!report.frida.server_version_verified);
        assert!(report.frida.server_version.is_none() && report.frida.version_match.is_none());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        probe.assert_read_only();
        elf(&binary, 183, 1, 1);
        assert!(inspect_server(&binary, Some("arm64-v8a")).is_err());
        fs::write(&binary, b"not an elf").unwrap();
        assert!(inspect_server(&binary, Some("arm64-v8a")).is_err());
    }

    #[tokio::test]
    async fn recovery_ownership_is_exact_and_preserves_saved_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let bytes = serde_json::to_vec_pretty(&saved_state()).unwrap();
        fs::write(manager.recovery_path(), &bytes).unwrap();
        let mut probe = FakeProbe::android();
        probe.set(
            Some("fixture-01"),
            &["reverse", "--list"],
            Ok("UsbFfs tcp:17890 tcp:17890\nUsbFfs tcp:17891 tcp:17891\nUsbFfs tcp:123 tcp:456"),
        );
        probe.set(Some("fixture-01"), &["forward", "--list"], Ok("fixture-01 tcp:42123 tcp:24073\nfixture-01 tcp:42124 tcp:27042\nfixture-02 tcp:42123 tcp:24073"));
        let report = manager.collect_diagnostics(None, &probe).await.unwrap();
        assert!(report.recovery.valid && report.recovery.needs_cleanup);
        assert!(!report.ready_for_proxy && !report.ready_for_frida);
        assert_eq!(report.mappings.reverse.len(), 3);
        assert_eq!(report.mappings.forward.len(), 2);
        assert_eq!(report.mappings.owned.len(), 2);
        assert_eq!(report.recovery.server_pid, Some(481));
        assert_eq!(fs::read(manager.recovery_path()).unwrap(), bytes);
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn corrupt_or_injected_recovery_is_reported_without_trusting_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let mut probe = FakeProbe::android();
        probe.set(
            Some("fixture-01"),
            &["reverse", "--list"],
            Ok("UsbFfs tcp:17890 tcp:17890"),
        );
        for bytes in [b"not json".to_vec(), {
            let mut saved = saved_state();
            saved.serial = "fixture-01; reboot".into();
            serde_json::to_vec(&saved).unwrap()
        }] {
            fs::write(manager.recovery_path(), &bytes).unwrap();
            let report = manager.collect_diagnostics(None, &probe).await.unwrap();
            assert!(!report.recovery.valid && report.recovery.needs_cleanup);
            assert!(report.recovery.serial.is_none());
            assert!(report.mappings.owned.is_empty());
            assert!(!report.ready_for_proxy && !report.ready_for_frida);
            assert_eq!(check(&report, "recovery.state").status, "error");
            assert_eq!(fs::read(manager.recovery_path()).unwrap(), bytes);
        }
        probe.assert_read_only();
    }

    #[tokio::test]
    async fn invalid_selection_is_rejected_before_any_probe() {
        let dir = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(dir.path().into());
        let probe = FakeProbe::android();
        assert!(manager
            .collect_diagnostics(Some("bad;reboot".into()), &probe)
            .await
            .is_err());
        assert!(probe.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn root_identity_requires_exact_uid_and_remote_paths_reject_shell_syntax() {
        assert!(is_root("uid=0(root) gid=0(root)"));
        assert!(is_root("uid=0 gid=0"));
        assert!(!is_root("uid=01(root) gid=0(root)"));
        assert!(!is_root("notuid=0(root)"));
        for path in [
            "/tmp/a;reboot",
            "relative/server",
            "/",
            "/tmp/$(id)",
            "/tmp/a b",
        ] {
            assert!(!valid_device_path(Path::new(path)));
        }
        assert!(valid_device_path(Path::new("/data/local/tmp/frida-server")));
    }
}
