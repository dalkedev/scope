use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{oneshot, Mutex as AsyncMutex},
};
mod adb;
mod diagnostics;
pub use diagnostics::{
    AdbDiagnostics, AdbMapping, AndroidDiagnostics, DeviceDiagnostics, DiagnosticCheck,
    FridaDiagnostics, MappingDiagnostics, RecoveryDiagnostics,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub serial: String,
    pub model: String,
    pub state: String,
    pub rooted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub package: String,
    pub name: String,
    pub running: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub running: bool,
    pub serial: Option<String>,
    pub package: Option<String>,
    pub mode: Option<String>,
    pub message: String,
    pub logs: Vec<String>,
}
impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            running: false,
            serial: None,
            package: None,
            mode: None,
            message: "Connect an Android device with USB debugging enabled".into(),
            logs: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recovery {
    serial: String,
    package: String,
    mode: String,
    port: u16,
    previous_proxy: Option<String>,
    previous_reverse: Option<String>,
    forward_port: Option<u16>,
    server_pid: Option<u32>,
    ca_hash: Option<String>,
    root_shell: bool,
    #[serde(default)]
    target_spawn_pending: bool,
    #[serde(default)]
    target_pid: Option<u32>,
}

struct Active {
    recovery: Recovery,
    worker: Option<Worker>,
}
struct Inner {
    runtime: PathBuf,
    adb: PathBuf,
    status: Mutex<DeviceStatus>,
    operation: AsyncMutex<()>,
    active: AsyncMutex<Option<Active>>,
    worker_generation: AtomicU64,
    recovery_io: Mutex<()>,
}
#[derive(Clone)]
pub struct DeviceManager {
    inner: Arc<Inner>,
}

impl DeviceManager {
    pub fn new(runtime_dir: PathBuf) -> Self {
        let status = if runtime_dir.join("device-recovery.json").is_file() {
            let saved = fs::read(runtime_dir.join("device-recovery.json"))
                .ok()
                .and_then(|data| serde_json::from_slice::<Recovery>(&data).ok());
            DeviceStatus {
                serial: saved.as_ref().map(|state| state.serial.clone()),
                package: saved.as_ref().map(|state| state.package.clone()),
                mode: saved.map(|state| state.mode),
                message: "A previous device session needs cleanup. Reconnect its device and press Stop before starting again.".into(),
                ..Default::default()
            }
        } else {
            DeviceStatus::default()
        };
        Self {
            inner: Arc::new(Inner {
                runtime: runtime_dir,
                adb: adb::discover_adb(),
                status: Mutex::new(status),
                operation: AsyncMutex::new(()),
                active: AsyncMutex::new(None),
                worker_generation: AtomicU64::new(0),
                recovery_io: Mutex::new(()),
            }),
        }
    }

    pub fn status(&self) -> DeviceStatus {
        self.inner.status.lock().unwrap().clone()
    }

    fn log(&self, text: impl AsRef<str>) {
        let mut status = self.inner.status.lock().unwrap();
        if status.logs.len() == 200 {
            status.logs.remove(0);
        }
        status.logs.push(bounded_diagnostic(text.as_ref()));
    }

    fn log_worker(&self, generation: u64, text: impl AsRef<str>) {
        let mut status = self.inner.status.lock().unwrap();
        if self.inner.worker_generation.load(Ordering::SeqCst) == generation {
            if status.logs.len() == 200 {
                status.logs.remove(0);
            }
            status.logs.push(bounded_diagnostic(text.as_ref()));
        }
    }

    async fn command(&self, serial: &str, args: &[&str]) -> Result<String> {
        adb::run(&self.inner.adb, Some(serial), args).await
    }

    async fn rooted(&self, serial: &str) -> (bool, bool) {
        if diagnostics::is_root(
            &adb::run_timeout(
                &self.inner.adb,
                Some(serial),
                &["shell", "id"],
                Duration::from_secs(4),
            )
            .await
            .unwrap_or_default(),
        ) {
            return (true, true);
        }
        let result = adb::run_timeout(
            &self.inner.adb,
            Some(serial),
            &["shell", "su", "-c", "'id'"],
            Duration::from_secs(4),
        )
        .await
        .unwrap_or_default();
        (diagnostics::is_root(&result), false)
    }

    async fn root_command(&self, state: &Recovery, command: &str) -> Result<String> {
        if state.root_shell {
            self.command(&state.serial, &["shell", command]).await
        } else {
            self.command(&state.serial, &["shell", "su", "-c", &adb::quote(command)])
                .await
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        let text = adb::run(&self.inner.adb, None, &["devices", "-l"]).await?;
        let mut devices = adb::parse_devices(&text);
        for device in &mut devices {
            if device.state == "device" {
                device.rooted = self.rooted(&device.serial).await.0;
            }
        }
        Ok(devices)
    }

    pub async fn list_apps(&self, serial: &str) -> Result<Vec<App>> {
        adb::validate_serial(serial)?;
        let packages = self
            .command(serial, &["shell", "pm", "list", "packages"])
            .await?;
        let processes = self
            .command(serial, &["shell", "ps", "-A", "-o", "NAME"])
            .await
            .unwrap_or_default();
        let mut apps = adb::parse_apps(&packages, &processes);
        // Frida can provide real display labels without installing a helper APK.
        // Enumeration falls back to exact package IDs if a server is unavailable.
        self.materialize()?;
        let mut child = worker_command(&self.inner.runtime);
        child
            .args(["--serial", serial, "--list-apps"])
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let address = self
            .inner
            .active
            .lock()
            .await
            .as_ref()
            .and_then(|active| {
                (active.recovery.serial == serial)
                    .then_some(active.recovery.forward_port)
                    .flatten()
            })
            .map(|port| format!("127.0.0.1:{port}"));
        if let Some(address) = address {
            child.args(["--address", &address]);
        }
        if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(7), child.output()).await {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(labels) = value.get("apps").and_then(|v| v.as_array()) {
                            let labels: HashMap<_, _> = labels
                                .iter()
                                .filter_map(|app| Some((app.get("package")?.as_str()?, app)))
                                .collect();
                            for app in &mut apps {
                                if let Some(label) = labels.get(app.package.as_str()) {
                                    app.name = label
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(&app.package)
                                        .into();
                                    app.running = label
                                        .get("running")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(app.running);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(apps)
    }

    pub async fn start(
        &self,
        serial: String,
        package: String,
        mode: String,
        proxy_port: u16,
        ca_cert: PathBuf,
        profile: Vec<String>,
    ) -> Result<DeviceStatus> {
        let _operation = self.inner.operation.lock().await;
        adb::validate_serial(&serial)?;
        adb::validate_package(&package)?;
        if !["proxy", "frida"].contains(&mode.as_str()) {
            bail!("Mode must be proxy or frida");
        }
        let profile = normalize_profiles(profile)?;
        if proxy_port == 0 {
            bail!("Proxy port must be greater than zero");
        }
        if self.inner.active.lock().await.is_some() || self.recovery_path().exists() {
            bail!("Stop the existing device session first. A saved recovery file must be cleaned up before another session starts");
        }
        tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(("127.0.0.1", proxy_port)),
        )
        .await
        .context("Scope proxy is not accepting connections")?
        .context("Start the Scope proxy before device interception")?;
        if self.command(&serial, &["get-state"]).await.context("Selected device is disconnected, offline or unauthorized. Run connection diagnostics and accept its USB debugging prompt")? != "device" {
            bail!("Selected device is not ready. Run connection diagnostics before starting interception");
        }
        if !self
            .command(&serial, &["shell", "pm", "path", &package])
            .await?
            .starts_with("package:")
        {
            bail!("The selected package is not installed");
        }
        let (rooted, root_shell) = self.rooted(&serial).await;
        if mode == "frida" && !rooted {
            bail!("Frida mode requires confirmed root access. Grant Shell superuser access on your rooted lab device, or choose system proxy mode for HTTP. Run connection diagnostics for the root result; no device settings were changed");
        }
        let frida_preflight = if mode == "frida" {
            let abi = self
                .command(&serial, &["shell", "getprop", "ro.product.cpu.abi"])
                .await?;
            if frida_arch(&abi).is_none() {
                bail!("Unsupported or unknown Android ABI {abi:?}. Frida mode needs arm64-v8a, armeabi-v7a, x86 or x86_64; system proxy mode does not require Frida. No device settings were changed");
            }
            let version = self.worker_version().await.map_err(|error| {
                let setup = self.setup_command(&abi);
                anyhow::anyhow!("Optional Frida runtime is unavailable: {error:#}\nRun this once in your terminal, then try Start again:\n{setup}\nThis creates Scope's private runtime; no Android settings have changed.")
            })?;
            let server = diagnostics::server_selection(&self.inner.runtime, Some(&abi));
            if let Some(path) = server.path.filter(|_| server.local) {
                diagnostics::inspect_server(&path, Some(&abi))
                    .context("The selected Frida server cannot run on this Android ABI; no device settings were changed")?;
            }
            if let Some(path) = env::var_os("SCOPE_FRIDA_SERVER") {
                if !diagnostics::valid_device_path(std::path::Path::new(&path)) {
                    bail!("SCOPE_FRIDA_SERVER must be an absolute device path without shell characters; no device settings were changed");
                }
            }
            Some((abi, version))
        } else {
            None
        };
        let previous_reverse = self
            .command(&serial, &["reverse", "--list"])
            .await?
            .lines()
            .find_map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                (fields.len() >= 3 && fields[1] == format!("tcp:{proxy_port}"))
                    .then(|| fields[2].to_string())
            });
        let previous_proxy = if mode == "proxy" {
            Some(
                self.command(
                    &serial,
                    &["shell", "settings", "get", "global", "http_proxy"],
                )
                .await?,
            )
        } else {
            None
        };
        let mut state = Recovery {
            serial: serial.clone(),
            package: package.clone(),
            mode: mode.clone(),
            port: proxy_port,
            previous_proxy,
            previous_reverse,
            forward_port: None,
            server_pid: None,
            ca_hash: None,
            root_shell,
            target_spawn_pending: false,
            target_pid: None,
        };
        *self.inner.status.lock().unwrap() = DeviceStatus {
            running: false,
            serial: Some(serial),
            package: Some(package),
            mode: Some(mode.clone()),
            message: "Preparing device interception".into(),
            logs: vec![],
        };
        self.materialize()?;
        let generation = self.inner.worker_generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.save_recovery(&state)?;
        let setup = async {
            self.command(&state.serial, &["reverse", &format!("tcp:{proxy_port}"), &format!("tcp:{proxy_port}")]).await?;
            self.log(format!("ADB reverse: device 127.0.0.1:{proxy_port} → Scope localhost:{proxy_port}"));
            let worker = if mode == "proxy" {
                self.log("Proxy mode is DEVICE-WIDE. The selected package is launched, but Android may route other applications through this proxy.");
                self.log("HTTPS requires manually configured application trust. System proxy mode does not install a CA or bypass pinning.");
                self.command(&state.serial, &["shell", "settings", "put", "global", "http_proxy", &format!("127.0.0.1:{proxy_port}")]).await?;
                self.launch(&state).await?;
                (None, None)
            } else {
                self.log("Frida mode instruments the selected application process only; Java HTTP stacks are supported. Native TLS, child processes and QUIC are not guaranteed.");
                let (abi, version) = frida_preflight.as_ref().context("Missing Frida preflight")?;
                let address = self.prepare_frida_server(&mut state, abi, version).await?;
                self.save_recovery(&state)?;
                let worker = self.spawn_worker(&mut state, &ca_cert, &profile, address, generation).await?;
                let live = worker.live.clone();
                state = worker.observation.lock().unwrap().recovery.clone();
                (Some(worker), Some(live))
            };
            Ok::<_, anyhow::Error>(worker)
        }.await;
        match setup {
            Ok((worker, live)) => {
                *self.inner.active.lock().await = Some(Active {
                    recovery: state,
                    worker,
                });
                let mut status = self.inner.status.lock().unwrap();
                status.running = live.as_ref().is_none_or(|live| live.load(Ordering::SeqCst));
                if !status.running {
                    status.message = "Frida worker exited just after startup. Press Stop to restore owned ADB mappings, then inspect connection diagnostics.".into();
                    bail!("Frida worker exited just after hook readiness; press Stop to restore the recorded device state");
                }
                status.message = if mode == "proxy" {
                    "Capturing device-wide traffic through Android's global proxy".into()
                } else {
                    "Selected app instrumented with experimental Frida Java hooks".into()
                };
                Ok(status.clone())
            }
            Err(error) => {
                self.log(format!("Setup failed: {error:#}"));
                let cleanup = if state.target_spawn_pending {
                    Err(anyhow::anyhow!("Target process cleanup is unconfirmed; recovery retained. Press Stop to reconcile the recorded PID, or manually reconcile an unknown spawn"))
                } else {
                    self.cleanup(&state).await
                };
                let mut status = self.inner.status.lock().unwrap();
                status.running = false;
                status.message = match cleanup {
                    Ok(_) => "Setup failed; device settings restored".into(),
                    Err(ref e) => format!(
                        "Setup failed; reconnect device and press Stop to finish cleanup: {e:#}"
                    ),
                };
                Err(error)
            }
        }
    }

    async fn launch(&self, state: &Recovery) -> Result<()> {
        let component = self
            .command(
                &state.serial,
                &[
                    "shell",
                    "cmd",
                    "package",
                    "resolve-activity",
                    "--brief",
                    "-a",
                    "android.intent.action.MAIN",
                    "-c",
                    "android.intent.category.LAUNCHER",
                    &state.package,
                ],
            )
            .await?;
        let component = component.lines().last().unwrap_or("").trim();
        if !component.starts_with(&format!("{}/", state.package))
            || !component
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.$/".contains(&c))
        {
            bail!("Selected app has no matching launcher activity");
        }
        self.command(
            &state.serial,
            &["shell", "am", "force-stop", &state.package],
        )
        .await?;
        let result = self
            .command(
                &state.serial,
                &["shell", "am", "start", "-W", "-n", component],
            )
            .await?;
        if result.contains("Error:") {
            bail!("Android could not launch application: {result}");
        }
        self.log(format!("Launched {}", state.package));
        Ok(())
    }

    async fn prepare_frida_server(
        &self,
        state: &mut Recovery,
        abi: &str,
        expected_version: &str,
    ) -> Result<Option<String>> {
        let selected = diagnostics::server_selection(&self.inner.runtime, Some(abi));
        if selected.source == "existingUsb" {
            self.log("Connecting to an existing Frida USB server. Set SCOPE_FRIDA_SERVER_LOCAL to provision a matching server, or SCOPE_FRIDA_SERVER to an existing device binary.");
            return Ok(None);
        }
        let remote = env::var("SCOPE_FRIDA_SERVER").ok().unwrap_or_else(|| {
            if selected.local {
                "/data/local/tmp/scope-frida-server".into()
            } else {
                selected
                    .path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }
        });
        if !diagnostics::valid_device_path(std::path::Path::new(&remote)) {
            bail!("SCOPE_FRIDA_SERVER must be an absolute device path without shell characters");
        }
        if let Some(local) = selected.path.filter(|_| selected.local) {
            diagnostics::inspect_server(&local, Some(abi))?;
            self.command(&state.serial, &["push", &local.to_string_lossy(), &remote])
                .await?;
            self.command(&state.serial, &["shell", "chmod", "755", &remote])
                .await?;
        }
        let version = self
            .root_command(state, &format!("{remote} --version"))
            .await?;
        if version.trim() != expected_version {
            bail!(
                "Frida version mismatch: device {} versus worker {}. Install exact matching versions or remove conflicting SCOPE_FRIDA_* overrides",
                version.trim(),
                expected_version
            );
        }
        let port = 24073;
        let sockets = self
            .root_command(state, "cat /proc/net/tcp")
            .await
            .context("Cannot inspect Android TCP sockets before starting the owned Frida server")?;
        let sockets6 = self
            .root_command(state, "cat /proc/net/tcp6 2>/dev/null || true")
            .await?;
        if tcp_port_listening(&sockets, port) || tcp_port_listening(&sockets6, port) {
            bail!("Android TCP port {port} already has a listener. Scope will not replace or stop an unrelated server. Stop your own listener or use an existing default-port USB Frida server, then retry");
        }
        // Only this spawned PID is owned and stopped by Scope. Existing servers are left alone.
        let pid = self.root_command(state, &format!("{remote} -l 127.0.0.1:{port} >/data/local/tmp/scope-frida-server.log 2>&1 & echo $!")).await?;
        let pid: u32 = pid
            .lines()
            .last()
            .context("Missing Frida server PID")?
            .parse()
            .context("Invalid Frida server PID")?;
        if pid <= 1 {
            bail!("Frida server returned an unsafe process identifier");
        }
        state.server_pid = Some(pid);
        self.save_recovery(state)?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !self
            .root_command(state, &format!("kill -0 {pid} && cat /proc/{pid}/cmdline"))
            .await
            .is_ok_and(|command| command.contains("127.0.0.1:24073"))
        {
            let log = self
                .root_command(
                    state,
                    "tail -c 2000 /data/local/tmp/scope-frida-server.log 2>/dev/null || true",
                )
                .await
                .unwrap_or_default();
            bail!("The owned Frida server exited before it became available. Check the selected ABI, Android compatibility and port 24073. Device log: {log}");
        }
        let forward = self
            .command(&state.serial, &["forward", "tcp:0", &format!("tcp:{port}")])
            .await?;
        let forward: u16 = forward.parse().context("Invalid forwarded Frida port")?;
        if forward == 0 {
            bail!("ADB returned an invalid zero Frida forwarding port");
        }
        state.forward_port = Some(forward);
        self.save_recovery(state)?;
        self.log(format!(
            "Started matching Frida server {version} with a dedicated local ADB forward"
        ));
        Ok(Some(format!("127.0.0.1:{forward}")))
    }

    async fn spawn_worker(
        &self,
        state: &mut Recovery,
        cert: &PathBuf,
        profile: &[String],
        address: Option<String>,
        generation: u64,
    ) -> Result<Worker> {
        let mut command = worker_command(&self.inner.runtime);
        command
            .args([
                "--serial",
                &state.serial,
                "--package",
                &state.package,
                "--ca",
            ])
            .arg(cert)
            .arg("--scripts")
            .arg(self.inner.runtime.join("frida"))
            .args(["--port", &state.port.to_string()]);
        if let Some(address) = address {
            command.args(["--address", &address]);
        }
        for profile in profile {
            command.args(["--profile", profile]);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        // Persist uncertainty before a worker can attempt a remote spawn, including a host crash.
        state.target_spawn_pending = true;
        self.save_recovery(state)?;
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                state.target_spawn_pending = false;
                self.save_recovery(state)?;
                return Err(error).context("Cannot start Frida worker; set SCOPE_PYTHON to a Python executable with frida installed");
            }
        };
        let stdout = child.stdout.take().context("Missing Frida worker stdout")?;
        let stderr = child.stderr.take().context("Missing Frida worker stderr")?;
        let (sender, receiver) = oneshot::channel();
        let live = Arc::new(AtomicBool::new(true));
        let observation = Arc::new(Mutex::new(WorkerObservation::new(
            state.clone(),
            profile.to_vec(),
        )));
        let monitor = tokio::spawn(monitor_worker_output(
            self.clone(),
            stdout,
            generation,
            live.clone(),
            sender,
            observation.clone(),
        ));
        tokio::spawn(monitor_worker_stderr(self.clone(), stderr, generation));
        let mut worker = Worker {
            child,
            live,
            observation,
            monitor,
        };
        match tokio::time::timeout(Duration::from_secs(40), receiver).await {
            Ok(Ok(Ok(()))) => Ok(worker),
            result => {
                let teardown = self.finish_worker(&mut worker).await;
                *state = worker.observation.lock().unwrap().recovery.clone();
                self.invalidate_worker();
                if teardown.is_ok() {
                    state.target_spawn_pending = false;
                    self.save_recovery(state)?;
                }
                let cause = match result {
                    Ok(Ok(Err(message))) => message,
                    _ => "Frida worker did not confirm hook readiness within 40 seconds".into(),
                };
                bail!(
                    "{cause}; teardown: {}",
                    teardown
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "confirmed".into())
                );
            }
        }
    }

    // Never holds operation/recovery locks while awaiting stdout. The monitor must drain the final receipt.
    async fn finish_worker(&self, worker: &mut Worker) -> Result<()> {
        if let Some(mut stdin) = worker.child.stdin.take() {
            let _ = stdin.write_all(b"stop\n").await;
        }
        let exit = match tokio::time::timeout(Duration::from_secs(12), worker.child.wait()).await {
            Ok(result) => result.context("Cannot reap Frida worker"),
            Err(_) => {
                let _ = worker.child.kill().await;
                Err(anyhow::anyhow!("Frida worker exceeded 12-second cleanup grace; target termination is unconfirmed"))
            }
        };
        let drained = tokio::time::timeout(Duration::from_secs(2), &mut worker.monitor).await;
        if !matches!(drained, Ok(Ok(()))) {
            worker.monitor.abort();
            bail!("Frida worker output did not drain; target termination is unconfirmed");
        }
        let exit = exit?;
        if !exit.success() {
            bail!("Frida worker failed ({exit}); recovery retained");
        }
        let observed = worker.observation.lock().unwrap();
        observed.confirm_stopped()?;
        Ok(())
    }

    fn invalidate_worker(&self) {
        let _io = self.inner.recovery_io.lock().unwrap();
        self.inner.worker_generation.fetch_add(1, Ordering::SeqCst);
    }

    pub async fn stop(&self) -> Result<DeviceStatus> {
        let _operation = self.inner.operation.lock().await;
        let active = self.inner.active.lock().await.take();
        let state = if let Some(mut active) = active {
            if let Some(mut worker) = active.worker.take() {
                let result = self.finish_worker(&mut worker).await;
                active.recovery = worker.observation.lock().unwrap().recovery.clone();
                self.invalidate_worker();
                if let Err(error) = result {
                    self.cleanup_failed(&active.recovery, &error);
                    return Err(error);
                }
                active.recovery.target_spawn_pending = false;
                self.save_recovery(&active.recovery)?;
            }
            Some(active.recovery)
        } else if self.recovery_path().is_file() {
            Some(serde_json::from_slice::<Recovery>(&fs::read(
                self.recovery_path(),
            )?)?)
        } else {
            None
        };
        if let Some(state) = state {
            if let Err(error) = self.cleanup(&state).await {
                self.cleanup_failed(&state, &error);
                return Err(error);
            }
        }
        let mut status = self.inner.status.lock().unwrap();
        status.running = false;
        // An identity here signals unfinished cleanup to the UI. Clear it only
        // after termination and owned settings restoration have succeeded.
        status.serial = None;
        status.package = None;
        status.mode = None;
        status.message = "Device settings restored; instrumented process ended when applicable; saved captures remain available".into();
        Ok(status.clone())
    }

    fn cleanup_failed(&self, state: &Recovery, error: &anyhow::Error) {
        let mut status = self.inner.status.lock().unwrap();
        status.running = false;
        status.message = format!(
            "Cleanup incomplete. Reconnect {} and press Stop again: {error:#}",
            state.serial
        );
    }

    async fn reconcile_target(&self, state: &Recovery) -> Result<()> {
        if !state.target_spawn_pending {
            return Ok(());
        }
        let pid = state.target_pid.context("Spawn was attempted without a recorded PID. Manually reconcile the app/process on the device; Scope cannot safely infer or kill it. Recovery retained")?;
        if pid == 0 {
            bail!("Invalid recorded target PID; recovery retained");
        }
        // Require a readable root procfs view. A package pid lookup or worker exit cannot establish absence.
        let probe = format!(
            r#"if [ "$(id -u)" != 0 ] || [ ! -r /proc/1/stat ] || [ ! -r /proc/self/stat ]; then
 echo UNKNOWN
elif [ -d /proc/{pid} ]; then
 echo PRESENT
elif mounts=$(cat /proc/mounts 2>/dev/null) && entries=$(ls /proc 2>/dev/null); then
 case "$mounts" in
 *hidepid=*) echo UNKNOWN;;
 *) found=0; init=0
    for entry in $entries; do
     [ "$entry" = "{pid}" ] && found=1
     [ "$entry" = 1 ] && init=1
    done
    if [ "$found" = 1 ]; then echo PRESENT
    elif [ "$init" = 1 ] && [ ! -e /proc/{pid} ]; then echo ABSENT
    else echo UNKNOWN; fi;;
 esac
else echo UNKNOWN; fi"#
        );
        let result = self
            .root_command(state, &probe)
            .await
            .context("Cannot inspect recorded target process; recovery retained")?;
        if result.trim() != "ABSENT" {
            bail!("Recorded target PID {pid} is present or its state is inaccessible. Scope will not kill a saved or reused PID; manually reconcile it and retry Stop. Recovery retained");
        }
        self.log(format!(
            "Recorded target PID {pid} is absent in the observed root procfs view"
        ));
        Ok(())
    }

    async fn cleanup(&self, state: &Recovery) -> Result<()> {
        // Validate every persisted field before the first cleanup command.
        diagnostics::validate_recovery(state)?;
        self.reconcile_target(state).await?;
        let mut errors = vec![];
        if let Some(old) = &state.previous_proxy {
            let current = self
                .command(
                    &state.serial,
                    &["shell", "settings", "get", "global", "http_proxy"],
                )
                .await;
            let restore = match current {
                Ok(current) => {
                    let decision = restoration_decision(
                        normalize_proxy(&current),
                        &format!("127.0.0.1:{}", state.port),
                        normalize_proxy(old),
                    );
                    match decision {
                        Restoration::AlreadyRestored => {
                            self.log("Android proxy was already restored")
                        }
                        Restoration::PreserveExternal => self.log(
                            "Android proxy changed outside Scope; preserving the current setting",
                        ),
                        Restoration::Restore => {}
                    }
                    decision == Restoration::Restore
                }
                Err(error) => {
                    errors.push(format!("Cannot inspect current Android proxy: {error:#}"));
                    false
                }
            };
            if restore {
                let args = if old.is_empty() || old == "null" {
                    vec!["shell", "settings", "delete", "global", "http_proxy"]
                } else {
                    vec!["shell", "settings", "put", "global", "http_proxy", old]
                };
                // Quote existing user settings, which are not controlled by Scope.
                let args: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i == 5 { adb::quote(v) } else { v.to_string() })
                    .collect();
                let args: Vec<_> = args.iter().map(String::as_str).collect();
                if let Err(error) = self.command(&state.serial, &args).await {
                    errors.push(error.to_string());
                } else {
                    self.log("Previous Android proxy restored");
                }
            }
        }
        if let Some(hash) = &state.ca_hash {
            let script = format!("rm -f /system/etc/security/cacerts/{hash}.0 /apex/com.android.conscrypt/cacerts/{hash}.0 /data/local/tmp/scope-{hash}.0 /data/local/tmp/scope-install-ca.sh; for pid in $(pidof zygote zygote64 {}); do nsenter --mount=/proc/$pid/ns/mnt -- rm -f /apex/com.android.conscrypt/cacerts/{hash}.0 /system/etc/security/cacerts/{hash}.0 || exit 1; done", state.package);
            if let Err(error) = self.root_command(state, &script).await {
                errors.push(error.to_string());
            } else {
                self.log("Scope CA removed from shared trust stores and selected app; temporary mount overlays disappear after reboot");
            }
        }
        if let Some(port) = state.forward_port {
            match self.command(&state.serial, &["forward", "--list"]).await {
                Ok(mappings) => {
                    let local = format!("tcp:{port}");
                    let mappings =
                        diagnostics::parse_mappings(&mappings, "forward", &state.serial, None);
                    let current = mappings
                        .iter()
                        .find(|mapping| mapping.local == local)
                        .map(|mapping| mapping.remote.as_str());
                    if restoration_decision(current, "tcp:24073", None) == Restoration::Restore {
                        if let Err(error) = self
                            .command(
                                &state.serial,
                                &["forward", "--remove", &format!("tcp:{port}")],
                            )
                            .await
                        {
                            errors.push(error.to_string());
                        }
                    } else {
                        self.log("Owned Frida forward is absent or changed outside Scope; preserving other mappings");
                    }
                }
                Err(error) => {
                    errors.push(format!("Cannot inspect current ADB forwarding: {error:#}"))
                }
            }
        }
        if let Some(pid) = state.server_pid {
            let command = format!("if [ -r /proc/{pid}/cmdline ] && tr '\\0' ' ' </proc/{pid}/cmdline | grep -q '127.0.0.1:24073'; then kill {pid}; fi");
            if let Err(error) = self.root_command(state, &command).await {
                errors.push(error.to_string());
            }
        }
        let reverse = format!("tcp:{}", state.port);
        match self.command(&state.serial, &["reverse", "--list"]).await {
            Ok(mappings) => {
                let mappings =
                    diagnostics::parse_mappings(&mappings, "reverse", &state.serial, None);
                let current = mappings
                    .iter()
                    .find(|mapping| mapping.local == reverse)
                    .map(|mapping| mapping.remote.as_str());
                match restoration_decision(current, &reverse, state.previous_reverse.as_deref()) {
                    Restoration::Restore => {
                        let result = if let Some(old) = &state.previous_reverse {
                            self.command(&state.serial, &["reverse", &reverse, old])
                                .await
                        } else {
                            self.command(&state.serial, &["reverse", "--remove", &reverse])
                                .await
                        };
                        if let Err(error) = result {
                            errors.push(error.to_string());
                        }
                    }
                    Restoration::AlreadyRestored => {
                        self.log("ADB reverse mapping was already restored")
                    }
                    Restoration::PreserveExternal => self.log(
                        "ADB reverse mapping changed outside Scope; preserving the current mapping",
                    ),
                }
            }
            Err(error) => errors.push(format!(
                "Cannot inspect current ADB reverse mappings: {error:#}"
            )),
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("; "));
        }
        let _io = self.inner.recovery_io.lock().unwrap();
        if self.recovery_path().exists() {
            fs::remove_file(self.recovery_path())?;
        }
        Ok(())
    }

    fn recovery_path(&self) -> PathBuf {
        self.inner.runtime.join("device-recovery.json")
    }

    async fn worker_version(&self) -> Result<String> {
        diagnostics::runtime_version(&self.inner.runtime).await
    }

    fn worker_ended(&self, generation: u64, message: &str) {
        let mut status = self.inner.status.lock().unwrap();
        if self.inner.worker_generation.load(Ordering::SeqCst) == generation {
            status.running = false;
            status.message = message.into();
        }
    }

    fn setup_command(&self, abi: &str) -> String {
        let installer = resource_file("device/install-runtime.py")
            .unwrap_or_else(|| PathBuf::from("resources/device/install-runtime.py"));
        let interpreter = if cfg!(windows) { "py -3" } else { "python3" };
        format!(
            "{interpreter} {} --runtime-dir {} --abi {}",
            terminal_quote(&installer.to_string_lossy()),
            terminal_quote(&self.inner.runtime.to_string_lossy()),
            terminal_quote(abi)
        )
    }
    fn save_recovery(&self, state: &Recovery) -> Result<()> {
        let _io = self.inner.recovery_io.lock().unwrap();
        self.write_recovery(state)
    }

    fn write_recovery(&self, state: &Recovery) -> Result<()> {
        fs::create_dir_all(&self.inner.runtime)?;
        let temporary = self.inner.runtime.join("device-recovery.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(state)?)?;
        fs::rename(temporary, self.recovery_path())?;
        Ok(())
    }

    fn materialize(&self) -> Result<()> {
        fs::create_dir_all(self.inner.runtime.join("frida"))?;
        for (path, content) in ASSETS {
            fs::write(self.inner.runtime.join(path), content)?;
        }
        Ok(())
    }
}

const ENGINE: &str = "scope-java-v1";

const WORKER_LINE_LIMIT: usize = 65536;
const DIAGNOSTIC_LIMIT: usize = 4096;

fn bounded_diagnostic(text: &str) -> String {
    const MARKER: &str = " [truncated]";
    if text.len() <= DIAGNOSTIC_LIMIT {
        return text.to_owned();
    }
    let mut end = DIAGNOSTIC_LIMIT - MARKER.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut retained = String::with_capacity(DIAGNOSTIC_LIMIT);
    retained.push_str(&text[..end]);
    retained.push_str(MARKER);
    retained
}

enum WorkerLine {
    Text(String),
    Oversized,
    InvalidUtf8,
}

// Keep one bounded line plus the reader's fixed-size input buffer. Report overflow
// immediately, then discard through LF without retaining the oversized remainder.
struct WorkerLines<R> {
    reader: BufReader<R>,
    bytes: Vec<u8>,
    discarding: bool,
}
impl<R: tokio::io::AsyncRead + Unpin> WorkerLines<R> {
    fn new(reader: R) -> Self {
        Self {
            reader: BufReader::with_capacity(8192, reader),
            // One extra byte permits a CR immediately before LF at the exact limit.
            bytes: Vec::with_capacity(WORKER_LINE_LIMIT + 1),
            discarding: false,
        }
    }

    fn finish_line(&mut self) -> WorkerLine {
        if self.bytes.last() == Some(&b'\r') {
            self.bytes.pop();
        }
        let line = match std::str::from_utf8(&self.bytes) {
            Ok(text) => WorkerLine::Text(text.to_owned()),
            Err(_) => WorkerLine::InvalidUtf8,
        };
        self.bytes.clear();
        line
    }

    async fn next_line(&mut self) -> std::io::Result<Option<WorkerLine>> {
        loop {
            let available = self.reader.fill_buf().await?;
            if available.is_empty() {
                return if self.discarding || self.bytes.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(self.finish_line()))
                };
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let count = newline.unwrap_or(available.len());
            let consumed = count + usize::from(newline.is_some());
            if self.discarding {
                self.reader.consume(consumed);
                self.discarding = newline.is_none();
                continue;
            }
            let remaining = WORKER_LINE_LIMIT + 1 - self.bytes.len();
            self.bytes
                .extend_from_slice(&available[..count.min(remaining)]);
            let oversized = count > remaining
                || (self.bytes.len() > WORKER_LINE_LIMIT && self.bytes.last() != Some(&b'\r'));
            self.reader.consume(consumed);
            if oversized {
                self.bytes.clear();
                self.discarding = newline.is_none();
                return Ok(Some(WorkerLine::Oversized));
            }
            if newline.is_some() {
                return Ok(Some(self.finish_line()));
            }
        }
    }
}

async fn monitor_worker_stderr<R: tokio::io::AsyncRead + Unpin>(
    manager: DeviceManager,
    stderr: R,
    generation: u64,
) {
    let mut lines = WorkerLines::new(stderr);
    loop {
        match lines.next_line().await {
            Ok(Some(WorkerLine::Text(line))) => manager.log_worker(generation, line),
            Ok(Some(WorkerLine::Oversized)) => manager.log_worker(
                generation,
                "Frida worker stderr line exceeded 65536 bytes; discarded",
            ),
            Ok(Some(WorkerLine::InvalidUtf8)) => {
                manager.log_worker(generation, "Invalid UTF-8 in Frida worker stderr")
            }
            Ok(None) => break,
            Err(error) => {
                manager.log_worker(
                    generation,
                    format!("Cannot read Frida worker stderr: {error}"),
                );
                break;
            }
        }
    }
}

fn normalize_profiles(profiles: Vec<String>) -> Result<Vec<String>> {
    let profiles = if profiles.is_empty() {
        vec!["proxy".into(), "certificate".into(), "unpinning".into()]
    } else {
        profiles
    };
    let mut selected = Vec::new();
    for profile in profiles {
        if !["proxy", "certificate", "unpinning"].contains(&profile.as_str()) {
            bail!("Unsupported instrumentation profile: {profile}");
        }
        if !selected.contains(&profile) {
            selected.push(profile);
        }
    }
    if selected.iter().any(|p| p == "unpinning") && !selected.iter().any(|p| p == "certificate") {
        bail!("unpinning profile requires certificate");
    }
    // Match the worker/engine canonical order while retaining exactly the explicit selection.
    Ok(["proxy", "certificate", "unpinning"]
        .into_iter()
        .filter(|profile| selected.iter().any(|selected| selected == profile))
        .map(String::from)
        .collect())
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WorkerEvent {
    Spawned {
        engine: String,
        pid: u32,
    },
    Ready {
        engine: String,
        pid: u32,
        profiles: Vec<String>,
        capabilities: HashMap<String, Vec<String>>,
    },
    Stopped {
        engine: String,
        pid: Option<u32>,
        #[serde(rename = "spawnAttempted")]
        spawn_attempted: bool,
        #[serde(rename = "cleanupComplete")]
        cleanup_complete: bool,
        #[serde(rename = "processTerminated")]
        process_terminated: bool,
    },
    Error {
        message: String,
    },
    Detached {
        message: String,
    },
    Log {
        message: serde_json::Value,
    },
}

struct Worker {
    child: Child,
    live: Arc<AtomicBool>,
    observation: Arc<Mutex<WorkerObservation>>,
    monitor: tokio::task::JoinHandle<()>,
}
struct WorkerObservation {
    recovery: Recovery,
    profiles: Vec<String>,
    spawned: bool,
    terminal: bool,
    failure: Option<String>,
    stopped: bool,
}
impl WorkerObservation {
    fn new(recovery: Recovery, profiles: Vec<String>) -> Self {
        Self {
            recovery,
            profiles,
            spawned: false,
            terminal: false,
            failure: None,
            stopped: false,
        }
    }
    fn confirm_stopped(&self) -> Result<()> {
        if let Some(error) = &self.failure {
            bail!("{error}; recovery retained");
        }
        if !self.stopped {
            bail!("Missing matching target-termination acknowledgment; recovery retained");
        }
        Ok(())
    }
    fn ready(
        &self,
        engine: &str,
        pid: u32,
        profiles: &[String],
        capabilities: &HashMap<String, Vec<String>>,
    ) -> bool {
        !self.terminal
            && self.spawned
            && engine == ENGINE
            && self.recovery.target_pid == Some(pid)
            && profiles == self.profiles
            && capabilities.len() == profiles.len()
            && profiles.iter().all(|profile| {
                let required = match profile.as_str() {
                    "proxy" => "java-url",
                    "certificate" => "sslcontext-scope-ca",
                    "unpinning" => "okhttp3-certificate-pinner",
                    _ => return false,
                };
                capabilities
                    .get(profile)
                    .is_some_and(|installed| installed.iter().any(|item| item == required))
            })
    }
}

async fn monitor_worker_output<R: tokio::io::AsyncRead + Unpin>(
    manager: DeviceManager,
    stdout: R,
    generation: u64,
    live: Arc<AtomicBool>,
    sender: oneshot::Sender<std::result::Result<(), String>>,
    observation: Arc<Mutex<WorkerObservation>>,
) {
    let mut sender = Some(sender);
    let mut lines = WorkerLines::new(stdout);
    loop {
        let line = lines.next_line().await;
        if matches!(line, Ok(None)) {
            break;
        }
        let io_failed = line.is_err();
        let _io = manager.inner.recovery_io.lock().unwrap();
        if manager.inner.worker_generation.load(Ordering::SeqCst) != generation {
            if io_failed {
                break;
            }
            continue;
        }
        let mut observed = observation.lock().unwrap();
        let event = match line {
            Ok(Some(WorkerLine::Text(line))) => serde_json::from_str::<WorkerEvent>(&line)
                .map_err(|_| "Invalid Frida worker protocol receipt".to_owned()),
            Ok(Some(WorkerLine::Oversized)) => {
                Err("Frida worker stdout line exceeded 65536 bytes; recovery retained".into())
            }
            Ok(Some(WorkerLine::InvalidUtf8)) => Err("Invalid UTF-8 in Frida worker stdout".into()),
            Err(error) => Err(format!("Cannot read Frida worker stdout: {error}")),
            Ok(None) => unreachable!(),
        };
        let mut error = None;
        match event {
            Ok(WorkerEvent::Spawned { engine, pid })
                if engine == ENGINE && pid > 0 && !observed.spawned && !observed.terminal =>
            {
                observed.recovery.target_pid = Some(pid);
                observed.spawned = true;
                if manager.recovery_path().is_file() {
                    if let Err(failure) = manager.write_recovery(&observed.recovery) {
                        error = Some(format!("Cannot persist spawned target PID: {failure}"));
                    }
                } else {
                    error =
                        Some("Recovery record disappeared before target PID persistence".into());
                }
            }
            Ok(WorkerEvent::Ready {
                engine,
                pid,
                profiles,
                capabilities,
            }) => {
                if observed.ready(&engine, pid, &profiles, &capabilities) {
                    if let Some(sender) = sender.take() {
                        let _ = sender.send(Ok(()));
                    }
                } else {
                    error = Some(
                        "Invalid engine readiness/capability receipt or terminal session".into(),
                    );
                }
            }
            Ok(WorkerEvent::Stopped {
                engine,
                pid,
                spawn_attempted,
                cleanup_complete,
                process_terminated,
            }) => {
                let valid = engine == ENGINE
                    && cleanup_complete
                    && if spawn_attempted {
                        observed.spawned
                            && pid == observed.recovery.target_pid
                            && process_terminated
                    } else {
                        !observed.spawned && pid.is_none() && !process_terminated
                    };
                if valid {
                    observed.stopped = true;
                } else {
                    error = Some("Invalid target-termination acknowledgment".into());
                }
                observed.terminal = true;
                if let Some(sender) = sender.take() {
                    let _ = sender.send(Err("Frida stopped before hook readiness".into()));
                }
            }
            Ok(WorkerEvent::Error { message }) => error = Some(message),
            Ok(WorkerEvent::Detached { message }) => {
                observed.terminal = true;
                if let Some(sender) = sender.take() {
                    let _ = sender.send(Err(format!(
                        "Frida detached before hook readiness: {message}"
                    )));
                }
            }
            Ok(WorkerEvent::Log { message }) => {
                manager.log(message.as_str().unwrap_or("Frida diagnostic"))
            }
            Err(message) => error = Some(message),
            _ => error = Some("Invalid Frida worker protocol receipt".into()),
        }
        if let Some(message) = error {
            observed.terminal = true;
            observed
                .failure
                .get_or_insert_with(|| bounded_diagnostic(&message));
            if let Some(sender) = sender.take() {
                let _ = sender.send(Err(message.clone()));
            }
            manager.log(message);
        }
        if observed.terminal {
            live.store(false, Ordering::SeqCst);
            manager.worker_ended(
                generation,
                "Frida worker ended; press Stop to reconcile the recorded device state",
            );
        }
        if io_failed {
            break;
        }
    }
    if let Some(sender) = sender.take() {
        let _ = sender.send(Err("Frida worker exited before hooks became ready".into()));
    }
    live.store(false, Ordering::SeqCst);
    manager.worker_ended(generation, "Frida worker exited; press Stop to restore the recorded device state and inspect the local worker log");
}

#[derive(Debug, PartialEq, Eq)]
enum Restoration {
    Restore,
    AlreadyRestored,
    PreserveExternal,
}
fn restoration_decision(current: Option<&str>, owned: &str, previous: Option<&str>) -> Restoration {
    if current == Some(owned) {
        Restoration::Restore
    } else if current == previous {
        Restoration::AlreadyRestored
    } else {
        Restoration::PreserveExternal
    }
}
fn normalize_proxy(proxy: &str) -> Option<&str> {
    let proxy = proxy.trim();
    (!proxy.is_empty() && proxy != "null").then_some(proxy)
}

fn tcp_port_listening(sockets: &str, port: u16) -> bool {
    sockets.lines().any(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields.len() >= 4
            && fields[3] == "0A"
            && fields[1]
                .rsplit(':')
                .next()
                .and_then(|hex| u16::from_str_radix(hex, 16).ok())
                == Some(port)
    })
}

fn python() -> std::ffi::OsString {
    env::var_os("SCOPE_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}

fn resource_file(relative: &str) -> Option<PathBuf> {
    let root = env::var_os("SCOPE_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let path = root.join(relative);
    path.is_file().then_some(path)
}

fn worker_command(runtime: &std::path::Path) -> Command {
    let binary = env::var_os("SCOPE_FRIDA_WORKER")
        .map(PathBuf::from)
        .or_else(|| {
            if env::var_os("SCOPE_PYTHON").is_some() {
                None
            } else {
                resource_file(if cfg!(windows) {
                    "bin/scope-frida-worker.exe"
                } else {
                    "bin/scope-frida-worker"
                })
            }
        });
    let command = if let Some(binary) = binary {
        Command::new(binary)
    } else {
        let managed = runtime.join("managed-frida/venv").join(if cfg!(windows) {
            "Scripts/python.exe"
        } else {
            "bin/python"
        });
        let interpreter = env::var_os("SCOPE_PYTHON").unwrap_or_else(|| {
            if managed.is_file() {
                managed.into_os_string()
            } else {
                python()
            }
        });
        let mut command = Command::new(interpreter);
        command.arg(runtime.join("frida_worker.py"));
        command
    };
    #[cfg(windows)]
    let mut command = command;
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}

fn terminal_quote(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        adb::quote(value)
    }
}

fn frida_arch(abi: &str) -> Option<&'static str> {
    match abi.trim() {
        "arm64-v8a" => Some("arm64"),
        "armeabi-v7a" => Some("arm"),
        "x86" => Some("x86"),
        "x86_64" => Some("x86_64"),
        _ => None,
    }
}

const ASSETS: &[(&str, &str)] = &[
    (
        "frida_worker.py",
        include_str!("../../../resources/device/frida_worker.py"),
    ),
    (
        "frida/scope-engine.js",
        include_str!("../../../resources/frida/scope-engine.js"),
    ),
    (
        "frida/frida-java-bridge.js",
        include_str!("../../../resources/frida/frida-java-bridge.js"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    fn test_observation() -> Arc<Mutex<WorkerObservation>> {
        Arc::new(Mutex::new(WorkerObservation::new(
            Recovery {
                serial: "fixture-01".into(),
                package: "com.scope.demo".into(),
                mode: "frida".into(),
                port: 17890,
                previous_proxy: None,
                previous_reverse: None,
                forward_port: None,
                server_pid: None,
                ca_hash: None,
                root_shell: true,
                target_spawn_pending: true,
                target_pid: None,
            },
            vec!["proxy".into()],
        )))
    }
    #[test]
    fn retained_diagnostics_are_byte_bounded_on_utf8_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        manager.log("a".repeat(4096));
        assert_eq!(manager.status().logs[0], "a".repeat(4096));
        manager.log("b".repeat(4097));
        assert_eq!(manager.status().logs[1].len(), 4096);
        assert!(manager.status().logs[1].ends_with(" [truncated]"));
        for _ in 0..205 {
            manager.log("😀".repeat(4096));
        }
        let status = manager.status();
        assert_eq!(status.logs.len(), 200);
        assert!(status.logs.iter().all(|line| line.len() <= 4096));
        assert!(status.logs.iter().all(|line| line.ends_with("[truncated]")));
        assert!(status.logs.iter().map(String::len).sum::<usize>() <= 819200);
        assert!(manager
            .inner
            .status
            .lock()
            .unwrap()
            .logs
            .iter()
            .all(|line| line.capacity() <= 4096));
        manager.log_worker(0, "é".repeat(4096));
        assert!(manager.status().logs.last().unwrap().len() <= 4096);
        let mut oversized_allocation = String::with_capacity(1024 * 1024);
        oversized_allocation.push_str("short");
        manager.log(&oversized_allocation);
        manager.log_worker(0, oversized_allocation);
        let retained = manager.inner.status.lock().unwrap();
        assert_eq!(retained.logs.last().unwrap(), "short");
        assert!(retained.logs.iter().all(|line| line.capacity() <= 4096));
        assert!(retained.logs.iter().map(String::capacity).sum::<usize>() <= 819200);
    }

    async fn monitor_fixture(
        input: Vec<u8>,
        fragment: usize,
    ) -> (
        DeviceManager,
        Arc<Mutex<WorkerObservation>>,
        std::result::Result<(), String>,
        tempfile::TempDir,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let observation = test_observation();
        manager
            .save_recovery(&observation.lock().unwrap().recovery)
            .unwrap();
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (sender, receiver) = oneshot::channel();
        let task = tokio::spawn(monitor_worker_output(
            manager.clone(),
            reader,
            0,
            Arc::new(AtomicBool::new(true)),
            sender,
            observation.clone(),
        ));
        for bytes in input.chunks(fragment) {
            writer.write_all(bytes).await.unwrap();
        }
        drop(writer);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        let readiness = receiver.await.unwrap();
        (manager, observation, readiness, directory)
    }

    const SPAWN_RECEIPT: &str = r#"{"kind":"spawned","engine":"scope-java-v1","pid":42}"#;
    const READY_RECEIPT: &str = r#"{"kind":"ready","engine":"scope-java-v1","pid":42,"profiles":["proxy"],"capabilities":{"proxy":["java-url"]}}"#;
    const STOP_RECEIPT: &str = r#"{"kind":"stopped","engine":"scope-java-v1","pid":42,"spawnAttempted":true,"cleanupComplete":true,"processTerminated":true}"#;

    fn padded_receipt(receipt: &str, size: usize) -> String {
        let prefix = format!("{},\"padding\":\"", receipt.strip_suffix('}').unwrap());
        format!("{prefix}{}\"}}", "x".repeat(size - prefix.len() - 2))
    }

    #[tokio::test]
    async fn oversized_stdout_is_terminal_even_before_newline() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let observation = test_observation();
        manager
            .save_recovery(&observation.lock().unwrap().recovery)
            .unwrap();
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (sender, receiver) = oneshot::channel();
        let task = tokio::spawn(monitor_worker_output(
            manager.clone(),
            reader,
            0,
            Arc::new(AtomicBool::new(true)),
            sender,
            observation.clone(),
        ));
        writer.write_all(&vec![b'x'; 65537]).await.unwrap();
        let ready = tokio::time::timeout(Duration::from_millis(300), receiver).await;
        task.abort();
        assert!(
            matches!(ready, Ok(Ok(Err(_)))),
            "oversize must fail without waiting for LF/EOF: {ready:?}"
        );
        assert!(manager.recovery_path().is_file());
        assert!(observation.lock().unwrap().confirm_stopped().is_err());
    }

    #[tokio::test]
    async fn oversized_stdout_cannot_become_ready_or_confirm_cleanup() {
        for oversized in [
            serde_json::json!({"kind":"log", "message":"x".repeat(1024 * 1024)}).to_string(),
            padded_receipt(READY_RECEIPT, 65537),
        ] {
            let input = format!("{SPAWN_RECEIPT}\n{oversized}\n{READY_RECEIPT}\n{STOP_RECEIPT}");
            let (manager, observation, readiness, _directory) =
                monitor_fixture(input.into_bytes(), 997).await;
            assert!(
                readiness.is_err(),
                "oversize may not be parsed as a valid receipt"
            );
            let observed = observation.lock().unwrap();
            assert!(
                observed.stopped,
                "bounded draining must resume at the next line"
            );
            assert!(observed.confirm_stopped().is_err());
            assert!(manager.recovery_path().is_file());
            let saved: Recovery =
                serde_json::from_slice(&fs::read(manager.recovery_path()).unwrap()).unwrap();
            assert!(saved.target_spawn_pending);
            assert_eq!(saved.target_pid, Some(42));
            assert!(manager.status().logs.iter().all(|line| line.len() <= 4096));
        }
    }

    #[tokio::test]
    async fn invalid_utf8_after_stopped_retains_failure() {
        let mut input = format!("{SPAWN_RECEIPT}\n{READY_RECEIPT}\n{STOP_RECEIPT}\n").into_bytes();
        input.extend_from_slice(b"\xff\n");
        let (_, observation, readiness, _directory) = monitor_fixture(input, 17).await;
        assert!(readiness.is_ok());
        assert!(observation.lock().unwrap().confirm_stopped().is_err());
    }

    #[tokio::test]
    async fn exact_frame_limit_preserves_fragmented_crlf_and_final_receipts() {
        let ready = padded_receipt(READY_RECEIPT, 65536);
        let input = format!("{SPAWN_RECEIPT}\r\n{ready}\r\n{{\"kind\":\"log\",\"message\":\"é😀\"}}\r\n{STOP_RECEIPT}");
        let (manager, observation, readiness, _directory) =
            monitor_fixture(input.into_bytes(), 7).await;
        assert!(readiness.is_ok());
        observation.lock().unwrap().confirm_stopped().unwrap();
        assert_eq!(manager.status().logs, ["é😀"]);
    }

    #[tokio::test]
    async fn final_stop_receipt_limit_applies_with_lf_crlf_and_eof() {
        for ending in ["\n", "\r\n", ""] {
            for (size, accepted) in [(65536, true), (65537, false)] {
                let stopped = padded_receipt(STOP_RECEIPT, size);
                let input = format!("{SPAWN_RECEIPT}\n{READY_RECEIPT}\n{stopped}{ending}");
                let (_, observation, readiness, _directory) =
                    monitor_fixture(input.into_bytes(), 1023).await;
                assert!(readiness.is_ok());
                assert_eq!(
                    observation.lock().unwrap().confirm_stopped().is_ok(),
                    accepted,
                    "size={size}, ending={ending:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn stderr_discards_one_oversized_line_and_resumes_utf8_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let task = tokio::spawn(monitor_worker_stderr(manager.clone(), reader, 0));
        let mut input = vec![b'x'; 1024 * 1024];
        input.extend_from_slice(b"\n\xff\n");
        input.extend_from_slice("é😀\r\nlast diagnostic".as_bytes());
        for bytes in input.chunks(997) {
            writer.write_all(bytes).await.unwrap();
        }
        drop(writer);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        let logs = manager.status().logs;
        assert_eq!(
            logs.len(),
            4,
            "overflow must produce one diagnostic, not one per fragment"
        );
        assert!(logs[0].contains("exceeded 65536"));
        assert!(logs[1].contains("Invalid UTF-8"));
        assert_eq!(&logs[2..], ["é😀", "last diagnostic"]);
        assert!(logs.iter().all(|line| line.len() <= 4096));
        manager.inner.worker_generation.store(1, Ordering::SeqCst);
        monitor_worker_stderr(manager.clone(), b"stale\n".as_slice(), 0).await;
        assert_eq!(manager.status().logs, logs);
    }

    #[tokio::test]
    async fn stderr_no_newline_overflow_is_reported_before_eof() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let task = tokio::spawn(monitor_worker_stderr(manager.clone(), reader, 0));
        writer.write_all(&vec![b'x'; 65537]).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while manager.status().logs.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(manager.status().logs.len(), 1);
        writer.write_all(&vec![b'x'; 1024 * 1024]).await.unwrap();
        drop(writer);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(manager.status().logs.len(), 1);
    }

    #[tokio::test]
    async fn bounded_reader_keeps_fixed_storage_when_discarding_without_newline() {
        let (mut writer, reader) = tokio::io::duplex(4096);
        let writing = tokio::spawn(async move {
            for _ in 0..256 {
                writer.write_all(&[b'x'; 8192]).await.unwrap();
            }
        });
        let mut lines = WorkerLines::new(reader);
        assert!(matches!(
            lines.next_line().await.unwrap(),
            Some(WorkerLine::Oversized)
        ));
        assert!(lines.bytes.len() <= 65537);
        assert!(lines.bytes.capacity() <= 65537);
        assert!(lines.next_line().await.unwrap().is_none());
        writing.await.unwrap();
        assert!(lines.bytes.capacity() <= 65537);
    }

    struct ReadThenError {
        bytes: Vec<u8>,
        offset: usize,
    }
    impl tokio::io::AsyncRead for ReadThenError {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _context: &mut std::task::Context<'_>,
            buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let reader = self.get_mut();
            if reader.offset == reader.bytes.len() {
                return std::task::Poll::Ready(Err(std::io::Error::other("fixture read failure")));
            }
            let end = (reader.offset + buffer.remaining()).min(reader.bytes.len());
            buffer.put_slice(&reader.bytes[reader.offset..end]);
            reader.offset = end;
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn stdout_io_failure_after_stopped_cannot_confirm_cleanup() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let observation = test_observation();
        manager
            .save_recovery(&observation.lock().unwrap().recovery)
            .unwrap();
        let input = format!("{SPAWN_RECEIPT}\n{READY_RECEIPT}\n{STOP_RECEIPT}\n");
        let (sender, receiver) = oneshot::channel();
        monitor_worker_output(
            manager.clone(),
            ReadThenError {
                bytes: input.into_bytes(),
                offset: 0,
            },
            0,
            Arc::new(AtomicBool::new(true)),
            sender,
            observation.clone(),
        )
        .await;
        assert!(receiver.await.unwrap().is_ok());
        assert!(observation.lock().unwrap().confirm_stopped().is_err());
        assert!(manager.recovery_path().is_file());
        assert!(manager
            .status()
            .logs
            .iter()
            .any(|line| line.contains("fixture read failure")));
    }

    #[test]
    fn old_recovery_defaults_do_not_invent_target_ownership() {
        let state: Recovery = serde_json::from_str(r#"{"serial":"fixture-01","package":"com.scope.demo","mode":"proxy","port":17890,"previousProxy":null,"previousReverse":null,"forwardPort":null,"serverPid":null,"caHash":"1234abcd","rootShell":true}"#).unwrap();
        assert!(!state.target_spawn_pending);
        assert_eq!(state.target_pid, None);
        assert_eq!(state.ca_hash.as_deref(), Some("1234abcd"));
    }

    #[test]
    fn explicit_profiles_remain_exact_and_defaults_are_compatible() {
        assert_eq!(
            normalize_profiles(vec![]).unwrap(),
            ["proxy", "certificate", "unpinning"]
        );
        assert_eq!(
            normalize_profiles(vec!["proxy".into(), "proxy".into()]).unwrap(),
            ["proxy"]
        );
        assert_eq!(
            normalize_profiles(vec!["certificate".into()]).unwrap(),
            ["certificate"]
        );
        assert_eq!(
            normalize_profiles(vec!["certificate".into(), "proxy".into()]).unwrap(),
            ["proxy", "certificate"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_stop_drains_and_requires_matching_successful_receipts() {
        // Run actual child pipes and the production monitor/teardown, including final-line races.
        let owned = r#"{"kind":"stopped","engine":"scope-java-v1","pid":42,"spawnAttempted":true,"cleanupComplete":true,"processTerminated":true}"#;
        let safe = r#"{"kind":"stopped","engine":"scope-java-v1","pid":null,"spawnAttempted":false,"cleanupComplete":true,"processTerminated":false}"#;
        let spawned = r#"{"kind":"spawned","engine":"scope-java-v1","pid":42}"#;
        let wrong = owned.replace("42", "43");
        let error = r#"{"kind":"error","message":"cleanup uncertain"}"#;
        for (spawn, receipt, exit, success) in [
            (spawned, owned.to_owned(), 0, true),
            ("", safe.to_owned(), 0, true),
            (spawned, "".into(), 0, false),
            (spawned, wrong, 0, false),
            (spawned, safe.to_owned(), 0, false),
            (spawned, owned.to_owned(), 1, false),
            (spawned, format!("{error}\n{owned}"), 0, false),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(directory.path().into());
            let observation = test_observation();
            manager
                .save_recovery(&observation.lock().unwrap().recovery)
                .unwrap();
            let before_stop = if spawn.is_empty() {
                String::new()
            } else {
                format!("printf '%s\\n' {}; ", adb::quote(spawn))
            };
            let script = format!(
                "{before_stop}read reply; printf '%s\\n' {}; exit {exit}",
                adb::quote(&receipt)
            );
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(script)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let stdout = child.stdout.take().unwrap();
            let live = Arc::new(AtomicBool::new(true));
            let (sender, _receiver) = oneshot::channel();
            let monitor = tokio::spawn(monitor_worker_output(
                manager.clone(),
                stdout,
                0,
                live.clone(),
                sender,
                observation.clone(),
            ));
            let mut worker = Worker {
                child,
                live,
                observation,
                monitor,
            };
            let result = manager.finish_worker(&mut worker).await;
            assert_eq!(result.is_ok(), success, "{receipt}: {result:?}");
            let saved: Recovery =
                serde_json::from_slice(&fs::read(manager.recovery_path()).unwrap()).unwrap();
            assert!(
                saved.target_spawn_pending,
                "receipt alone must not clear recovery before exit/drain validation"
            );
            if !spawn.is_empty() {
                assert_eq!(saved.target_pid, Some(42));
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn full_worker_cleanup_budget_is_allowed_before_killing() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let observation = test_observation();
        manager
            .save_recovery(&observation.lock().unwrap().recovery)
            .unwrap();
        let mut child = Command::new("sh").args(["-c", "read reply; sleep 9; printf '%s\\n' '{\"kind\":\"stopped\",\"engine\":\"scope-java-v1\",\"pid\":null,\"spawnAttempted\":false,\"cleanupComplete\":true,\"processTerminated\":false}'"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let live = Arc::new(AtomicBool::new(true));
        let (sender, _receiver) = oneshot::channel();
        let monitor = tokio::spawn(monitor_worker_output(
            manager.clone(),
            stdout,
            0,
            live.clone(),
            sender,
            observation.clone(),
        ));
        let mut worker = Worker {
            child,
            live,
            observation,
            monitor,
        };
        manager.finish_worker(&mut worker).await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn recovery_only_accepts_observed_known_pid_absence_and_preserves_legacy_ca_cleanup() {
        use std::os::unix::fs::PermissionsExt;
        for (pid, proc_state, succeeds) in [
            (Some(42), "ABSENT", true),
            (Some(42), "PRESENT", false),
            (Some(42), "UNKNOWN", false),
            (None, "ABSENT", false),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let log = directory.path().join("commands.log");
            let adb_path = directory.path().join("fake-adb");
            fs::write(&adb_path, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \"$*\" in\n*'/proc/42'*) echo {proc_state};;\n*) exit 0;;\nesac\n", adb::quote(&log.to_string_lossy()))).unwrap();
            fs::set_permissions(&adb_path, fs::Permissions::from_mode(0o755)).unwrap();
            let mut manager = DeviceManager::new(directory.path().into());
            Arc::get_mut(&mut manager.inner).unwrap().adb = adb_path;
            let mut state = test_observation().lock().unwrap().recovery.clone();
            state.target_pid = pid;
            state.ca_hash = Some("1234abcd".into());
            manager.save_recovery(&state).unwrap();
            assert_eq!(manager.stop().await.is_ok(), succeeds);
            assert_eq!(manager.recovery_path().exists(), !succeeds);
            let commands = fs::read_to_string(&log).unwrap_or_default();
            assert!(!commands.contains("kill 42"));
            assert!(
                !commands.contains("pidof com.scope.demo"),
                "package PID lookup cannot prove target termination"
            );
            if succeeds {
                assert!(commands.contains("rm -f /system/etc/security/cacerts/1234abcd.0"));
            } else {
                assert!(!commands.contains("rm -f"));
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn inaccessible_pid_listed_in_procfs_is_not_treated_as_absent() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        for (name, source) in [
            ("id", "#!/bin/sh\necho 0\n".to_owned()),
            ("ls", "#!/bin/sh\nprintf '1\\n4294967295\\n'\n".to_owned()),
            (
                "adb",
                format!(
                    "#!/bin/sh\nPATH={}:$PATH\nexport PATH\nexec sh -c \"$4\"\n",
                    adb::quote(&directory.path().to_string_lossy())
                ),
            ),
        ] {
            let path = directory.path().join(name);
            fs::write(&path, source).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut manager = DeviceManager::new(directory.path().into());
        Arc::get_mut(&mut manager.inner).unwrap().adb = directory.path().join("adb");
        let mut state = test_observation().lock().unwrap().recovery.clone();
        state.target_pid = Some(u32::MAX);
        assert!(manager.reconcile_target(&state).await.is_err(), "a visible directory entry is evidence of presence even if stat cannot observe the target");
    }

    #[tokio::test]
    async fn terminal_error_cannot_be_revived_by_ready_or_late_spawn() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let observation = test_observation();
        manager
            .save_recovery(&observation.lock().unwrap().recovery)
            .unwrap();
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (sender, receiver) = oneshot::channel();
        let live = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn(monitor_worker_output(
            manager.clone(),
            reader,
            0,
            live.clone(),
            sender,
            observation.clone(),
        ));
        writer.write_all(b"{\"kind\":\"error\",\"message\":\"failed\"}\n{\"kind\":\"spawned\",\"engine\":\"scope-java-v1\",\"pid\":42}\n{\"kind\":\"ready\",\"engine\":\"scope-java-v1\",\"pid\":42,\"profiles\":[\"proxy\"],\"capabilities\":{\"proxy\":[\"java-url\"]}}\n").await.unwrap();
        drop(writer);
        assert!(receiver.await.unwrap().is_err());
        task.await.unwrap();
        assert!(!live.load(Ordering::SeqCst));
        assert!(!observation.lock().unwrap().spawned);
        let saved: Recovery =
            serde_json::from_slice(&fs::read(manager.recovery_path()).unwrap()).unwrap();
        assert!(saved.target_spawn_pending);
        assert_eq!(saved.target_pid, None);
    }

    #[tokio::test]
    async fn unsupported_profiles_fail_before_proxy_or_device_probes() {
        for profiles in [
            vec!["root"],
            vec!["system-ca"],
            vec!["unknown"],
            vec!["unpinning"],
        ] {
            let directory = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(directory.path().into());
            let error = manager
                .start(
                    "fixture-01".into(),
                    "com.scope.demo".into(),
                    "frida".into(),
                    1,
                    directory.path().join("missing-ca"),
                    profiles.into_iter().map(String::from).collect(),
                )
                .await
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("profile") || error.contains("requires certificate"),
                "{error}"
            );
            assert!(!manager.recovery_path().exists());
        }
    }

    #[test]
    fn materialized_worker_can_construct_only_the_owned_engine() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        manager.materialize().unwrap();
        assert!(directory.path().join("frida/scope-engine.js").is_file());
        assert!(directory
            .path()
            .join("frida/frida-java-bridge.js")
            .is_file());
        assert!(!directory.path().join("frida/config-template.js").exists());
        assert!(!directory.path().join("frida/android").exists());
    }

    #[tokio::test]
    async fn invalid_readiness_never_confirms_startup() {
        for receipt in [
            r#"{"kind":"ready"}"#,
            r#"{"kind":"ready","engine":"other","pid":42,"profiles":["proxy"],"capabilities":{"proxy":["java-url"]}}"#,
            r#"{"kind":"ready","engine":"scope-java-v1","pid":43,"profiles":["proxy"],"capabilities":{"proxy":["java-url"]}}"#,
            r#"{"kind":"ready","engine":"scope-java-v1","pid":42,"profiles":["certificate"],"capabilities":{"certificate":["sslcontext-scope-ca"]}}"#,
            r#"{"kind":"ready","engine":"scope-java-v1","pid":42,"profiles":["proxy"],"capabilities":{"proxy":[]}}"#,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(directory.path().into());
            let observation = test_observation();
            manager
                .save_recovery(&observation.lock().unwrap().recovery)
                .unwrap();
            let (mut writer, reader) = tokio::io::duplex(4096);
            let (sender, receiver) = oneshot::channel();
            let task = tokio::spawn(monitor_worker_output(
                manager,
                reader,
                0,
                Arc::new(AtomicBool::new(true)),
                sender,
                observation,
            ));
            writer.write_all(format!("{{\"kind\":\"spawned\",\"engine\":\"scope-java-v1\",\"pid\":42}}\n{receipt}\n").as_bytes()).await.unwrap();
            drop(writer);
            assert!(receiver.await.unwrap().is_err(), "{receipt}");
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn late_generation_cannot_replace_or_recreate_recovery() {
        for saved in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let manager = DeviceManager::new(directory.path().into());
            manager.inner.worker_generation.store(2, Ordering::SeqCst);
            if saved {
                manager
                    .save_recovery(&test_observation().lock().unwrap().recovery)
                    .unwrap();
            }
            let before = fs::read(manager.recovery_path()).ok();
            let (mut writer, reader) = tokio::io::duplex(4096);
            let (sender, _receiver) = oneshot::channel();
            let task = tokio::spawn(monitor_worker_output(
                manager.clone(),
                reader,
                1,
                Arc::new(AtomicBool::new(true)),
                sender,
                test_observation(),
            ));
            writer
                .write_all(b"{\"kind\":\"spawned\",\"engine\":\"scope-java-v1\",\"pid\":42}\n")
                .await
                .unwrap();
            drop(writer);
            task.await.unwrap();
            assert_eq!(fs::read(manager.recovery_path()).ok(), before);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_checks_live_mapping_ownership_before_mutations() {
        use std::os::unix::fs::PermissionsExt;
        for external_change in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let log = directory.path().join("commands.log");
            let fake_adb = directory.path().join("fake-adb");
            let (proxy, reverse, forward) = if external_change {
                ("127.0.0.1:29999", "tcp:19999", "tcp:27042")
            } else {
                ("127.0.0.1:17890", "tcp:17890", "tcp:24073")
            };
            fs::write(&fake_adb, format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \"$*\" in\n'-s fixture-01 shell settings get global http_proxy') printf '%s\\n' '{proxy}';;\n'-s fixture-01 forward --list') printf '%s\\n' 'fixture-01 tcp:42123 {forward}';;\n'-s fixture-01 reverse --list') printf '%s\\n' 'UsbFfs tcp:17890 {reverse}';;\n*) {};;\nesac\n",
                adb::quote(&log.to_string_lossy()),
                if external_change { "echo 'unexpected mutation' >&2; exit 9" } else { "exit 0" }
            )).unwrap();
            fs::set_permissions(&fake_adb, fs::Permissions::from_mode(0o755)).unwrap();
            let mut manager = DeviceManager::new(directory.path().to_owned());
            Arc::get_mut(&mut manager.inner).unwrap().adb = fake_adb;
            manager
                .save_recovery(&Recovery {
                    serial: "fixture-01".into(),
                    package: "com.scope.demo".into(),
                    mode: "proxy".into(),
                    port: 17890,
                    previous_proxy: Some(":0".into()),
                    previous_reverse: Some("tcp:18888".into()),
                    forward_port: Some(42123),
                    server_pid: None,
                    ca_hash: None,
                    root_shell: false,
                    target_spawn_pending: false,
                    target_pid: None,
                })
                .unwrap();
            {
                let mut status = manager.inner.status.lock().unwrap();
                status.running = true;
                status.serial = Some("fixture-01".into());
                status.package = Some("com.scope.demo".into());
                status.mode = Some("proxy".into());
            }
            manager.stop().await.unwrap();
            let stopped = manager.status();
            assert!(!stopped.running);
            assert_eq!(
                stopped.serial, None,
                "successful cleanup must allow a new connection"
            );
            assert_eq!(stopped.package, None);
            assert_eq!(stopped.mode, None);
            let commands = fs::read_to_string(log).unwrap();
            assert_eq!(commands.contains("settings put"), !external_change);
            assert_eq!(commands.contains("forward --remove"), !external_change);
            assert_eq!(
                commands.contains("reverse tcp:17890 tcp:18888"),
                !external_change
            );
            assert!(!manager.recovery_path().exists());
            if external_change {
                assert!(manager
                    .status()
                    .logs
                    .iter()
                    .any(|line| line.contains("changed outside Scope")));
            }
        }
    }
    #[test]
    fn owned_server_port_probe_distinguishes_listeners_from_other_connections() {
        let listening =
            " sl local_address rem_address st\n0: 0100007F:5E09 00000000:0000 0A 00000000:00000000";
        assert!(tcp_port_listening(listening, 24073));
        assert!(tcp_port_listening(
            "0: 00000000000000000000000001000000:5E09 00000000000000000000000000000000:0000 0A",
            24073
        ));
        assert!(!tcp_port_listening(
            "0: 0100007F:5E09 0100007F:8DC2 01",
            24073
        ));
        assert!(!tcp_port_listening(listening, 27042));
        assert!(!tcp_port_listening("malformed", 24073));
    }
    #[tokio::test]
    async fn worker_stdout_eof_after_ready_marks_capture_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        manager.inner.worker_generation.store(1, Ordering::SeqCst);
        manager.inner.status.lock().unwrap().running = true;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (sender, receiver) = oneshot::channel();
        let live = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn(monitor_worker_output(
            manager.clone(),
            reader,
            1,
            live.clone(),
            sender,
            test_observation(),
        ));
        manager
            .save_recovery(&test_observation().lock().unwrap().recovery)
            .unwrap();
        writer.write_all(b"{\"kind\":\"spawned\",\"engine\":\"scope-java-v1\",\"pid\":42}\n{\"kind\":\"ready\",\"engine\":\"scope-java-v1\",\"pid\":42,\"profiles\":[\"proxy\"],\"capabilities\":{\"proxy\":[\"java-url\"]}}\n").await.unwrap();
        assert!(receiver.await.unwrap().is_ok());
        drop(writer);
        task.await.unwrap();
        assert!(!live.load(Ordering::SeqCst));
        assert!(!manager.status().running);
        assert!(manager.status().message.contains("press Stop"));
    }
    #[test]
    fn stderr_waiting_for_status_lock_cannot_append_to_a_new_generation() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let mut status = manager.inner.status.lock().unwrap();
        let (started, waiting) = std::sync::mpsc::channel();
        let old_worker = manager.clone();
        let writer = std::thread::spawn(move || {
            started.send(()).unwrap();
            old_worker.log_worker(0, "old worker stderr");
        });
        waiting.recv().unwrap();
        // Hold the actual status lock while the old worker reaches its append boundary.
        std::thread::sleep(Duration::from_millis(20));
        manager.inner.worker_generation.store(1, Ordering::SeqCst);
        *status = DeviceStatus {
            message: "new session".into(),
            ..Default::default()
        };
        drop(status);
        writer.join().unwrap();
        assert!(
            manager.status().logs.is_empty(),
            "stale stderr must not enter the replacement status"
        );
        manager.log_worker(1, "current worker stderr");
        assert_eq!(manager.status().logs, ["current worker stderr"]);
    }

    #[tokio::test]
    async fn late_worker_eof_cannot_overwrite_a_new_session_status() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        manager.inner.worker_generation.store(2, Ordering::SeqCst);
        manager.inner.status.lock().unwrap().running = true;
        let (writer, reader) = tokio::io::duplex(4096);
        let (sender, _receiver) = oneshot::channel();
        let task = tokio::spawn(monitor_worker_output(
            manager.clone(),
            reader,
            1,
            Arc::new(AtomicBool::new(true)),
            sender,
            test_observation(),
        ));
        drop(writer);
        task.await.unwrap();
        assert!(manager.status().running);
    }
    #[tokio::test]
    async fn worker_detach_before_ready_reports_cause_without_waiting_for_eof() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().into());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (sender, receiver) = oneshot::channel();
        let task = tokio::spawn(monitor_worker_output(
            manager,
            reader,
            0,
            Arc::new(AtomicBool::new(true)),
            sender,
            test_observation(),
        ));
        writer
            .write_all(b"{\"kind\":\"detached\",\"message\":\"process-terminated\"}\n")
            .await
            .unwrap();
        let error = tokio::time::timeout(Duration::from_secs(1), receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(error.contains("process-terminated"));
        drop(writer);
        task.await.unwrap();
    }
    #[test]
    fn android_server_abi_mapping_is_explicit() {
        assert_eq!(frida_arch("arm64-v8a\n"), Some("arm64"));
        assert_eq!(frida_arch("armeabi-v7a"), Some("arm"));
        assert_eq!(frida_arch("x86_64"), Some("x86_64"));
        assert_eq!(frida_arch("unknown;id"), None);
    }
    #[test]
    fn loads_recovery_status_without_claiming_live_capture() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("device-recovery.json"), "{}").unwrap();
        let manager = DeviceManager::new(directory.path().to_owned());
        assert!(!manager.status().running);
        assert!(manager.status().message.contains("cleanup"));
    }

    #[tokio::test]
    async fn disconnected_cleanup_retains_recovery_for_retry() {
        let directory = tempfile::tempdir().unwrap();
        let state = Recovery {
            serial: "emulator-5554".into(),
            package: "com.scope.demo".into(),
            mode: "proxy".into(),
            port: 17890,
            previous_proxy: Some(":0".into()),
            previous_reverse: None,
            forward_port: None,
            server_pid: None,
            ca_hash: None,
            root_shell: false,
            target_spawn_pending: false,
            target_pid: None,
        };
        let mut manager = DeviceManager::new(directory.path().to_owned());
        Arc::get_mut(&mut manager.inner).unwrap().adb = directory.path().join("missing-adb");
        manager.save_recovery(&state).unwrap();
        {
            let mut status = manager.inner.status.lock().unwrap();
            status.running = true;
            status.serial = Some(state.serial.clone());
            status.package = Some(state.package.clone());
            status.mode = Some(state.mode.clone());
        }
        assert!(manager.stop().await.is_err());
        assert!(manager.recovery_path().exists());
        assert!(!manager.status().running);
        assert_eq!(manager.status().serial.as_deref(), Some("emulator-5554"));
        assert_eq!(manager.status().package.as_deref(), Some("com.scope.demo"));
        assert_eq!(manager.status().mode.as_deref(), Some("proxy"));
        assert!(manager.status().message.contains("Cleanup incomplete"));
    }

    #[tokio::test]
    async fn malformed_saved_package_is_rejected_before_remote_shell() {
        let directory = tempfile::tempdir().unwrap();
        let manager = DeviceManager::new(directory.path().to_owned());
        let state = Recovery {
            serial: "emulator-5554".into(),
            package: "com.scope;touch /bad".into(),
            mode: "proxy".into(),
            port: 17890,
            previous_proxy: Some(":0".into()),
            previous_reverse: None,
            forward_port: None,
            server_pid: None,
            ca_hash: None,
            root_shell: false,
            target_spawn_pending: false,
            target_pid: None,
        };
        manager.save_recovery(&state).unwrap();
        let error = manager.stop().await.unwrap_err().to_string();
        assert!(error.contains("Invalid Android application ID"));
        assert!(manager.recovery_path().exists());
    }
}
