use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROBE_CAP: usize = 8192;
const PERIOD: Duration = Duration::from_millis(10);

#[derive(Serialize)]
struct ProbeSample {
    // Timer deadlines are scheduled; network attempts have no fixed cadence.
    scheduled_ns: Option<u64>,
    start_ns: u64,
    end_ns: u64,
    elapsed_ns: u64,
    success: bool,
    in_window: bool,
}
#[derive(Serialize)]
struct ProbeValues {
    elapsed_ns: Vec<u64>,
    samples: Vec<ProbeSample>,
    failures: usize,
    dropped: usize,
    missed_periods: u64,
    in_window_samples: usize,
    window_status: &'static str,
}
impl Default for ProbeValues {
    fn default() -> Self {
        Self {
            elapsed_ns: Vec::new(),
            samples: Vec::new(),
            failures: 0,
            dropped: 0,
            missed_periods: 0,
            in_window_samples: 0,
            window_status: "NOTRUN",
        }
    }
}
impl ProbeValues {
    fn push(&mut self, sample: ProbeSample) {
        self.failures += usize::from(!sample.success);
        if self.samples.len() < PROBE_CAP {
            if sample.success {
                self.elapsed_ns.push(sample.elapsed_ns);
            }
            self.samples.push(sample);
        } else {
            self.dropped += 1;
        }
    }
    fn mark_window(&mut self, start: u64, end: u64) {
        for sample in &mut self.samples {
            sample.in_window = end > start
                && match sample.scheduled_ns {
                    // A delayed wake still measures the deadline inside the batch.
                    Some(scheduled) => start <= scheduled && scheduled < end,
                    // Include attempts started before the batch if service spans it.
                    None => sample.start_ns < end && sample.end_ns > start,
                };
        }
        self.in_window_samples = self.samples.iter().filter(|s| s.in_window).count();
        self.window_status = if self.dropped > 0 {
            "INVALID"
        } else if self.in_window_samples == 0 {
            "NOTRUN"
        } else {
            "PASS"
        };
    }
}

pub(super) struct Probes {
    epoch: Instant,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    timer: tokio::task::JoinHandle<ProbeValues>,
    server: tokio::task::JoinHandle<()>,
    network: std::thread::JoinHandle<ProbeValues>,
}
#[derive(Serialize)]
pub(super) struct ProbeReport {
    timer: ProbeValues,
    network: ProbeValues,
    period_ms: u64,
    cap_per_probe: usize,
    window_start_ns: u64,
    window_end_ns: u64,
}
impl ProbeReport {
    pub(super) fn detects_control(&self) -> bool {
        // Only in-window observations validate the intentional blocking control.
        self.timer
            .samples
            .iter()
            .any(|s| s.in_window && s.elapsed_ns > 100_000_000)
            && self
                .network
                .samples
                .iter()
                .any(|s| s.in_window && (!s.success || s.elapsed_ns > 100_000_000))
            && self.covered()
    }
    pub(super) fn covered(&self) -> bool {
        // Collector validity differs from coverage of a potentially sub-period batch.
        self.timer.dropped == 0 && self.network.dropped == 0
    }
}
impl Probes {
    pub(super) async fn start() -> Self {
        let epoch = Instant::now();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(false));
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut byte = [0u8; 1];
                // One finite probe at a time; no detached connection tasks.
                if matches!(
                    tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut byte))
                        .await,
                    Ok(Ok(_))
                ) {
                    let _ = stream.write_all(&byte).await;
                }
            }
        });
        let ready = Arc::new(AtomicBool::new(false));
        let (thread_stop, thread_active, thread_ready) =
            (stop.clone(), active.clone(), ready.clone());
        let network = std::thread::spawn(move || {
            let mut values = ProbeValues::default();
            while !thread_stop.load(Ordering::SeqCst) {
                let started = Instant::now();
                let result = echo(addr);
                let ended = Instant::now();
                if result.is_ok() {
                    thread_ready.store(true, Ordering::SeqCst);
                }
                if thread_active.load(Ordering::SeqCst) {
                    values.push(ProbeSample {
                        scheduled_ns: None,
                        start_ns: ns(started.duration_since(epoch)),
                        end_ns: ns(ended.duration_since(epoch)),
                        elapsed_ns: ns(ended.duration_since(started)),
                        success: result.is_ok(),
                        in_window: false,
                    });
                }
                std::thread::sleep(PERIOD);
            }
            values
        });
        // Arm only after the independent network probe is ready. Otherwise
        // readiness polling (notably Windows timer granularity) can leave an
        // already-overdue first deadline outside the upcoming control window.
        tokio::time::timeout(Duration::from_secs(3), async {
            while !ready.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("loopback probe readiness failed");
        let (timer_stop, timer_active) = (stop.clone(), active.clone());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let timer = tokio::spawn(async move {
            let mut values = ProbeValues::default();
            let mut deadline = tokio::time::Instant::now() + PERIOD;
            let _ = tx.send(());
            loop {
                tokio::time::sleep_until(deadline).await;
                let now = tokio::time::Instant::now();
                let lag = now.saturating_duration_since(deadline);
                let skipped = (lag.as_nanos() / PERIOD.as_nanos()) as u64;
                if timer_active.load(Ordering::SeqCst) {
                    let wake_ns = ns(now.into_std().duration_since(epoch));
                    values.push(ProbeSample {
                        scheduled_ns: Some(ns(deadline.into_std().duration_since(epoch))),
                        start_ns: wake_ns,
                        end_ns: wake_ns,
                        elapsed_ns: ns(lag),
                        success: true,
                        in_window: false,
                    });
                    values.missed_periods += skipped;
                }
                deadline += PERIOD * (skipped as u32 + 1);
                if timer_stop.load(Ordering::SeqCst) {
                    break;
                }
            }
            values
        });
        rx.await.unwrap();
        active.store(true, Ordering::SeqCst);
        Self {
            epoch,
            stop,
            active,
            timer,
            server,
            network,
        }
    }
    pub(super) async fn finish(self, batch_start: Instant, batch_end: Instant) -> ProbeReport {
        // Drain delayed observations; these remain distinguishable from the batch.
        tokio::time::sleep(PERIOD * 2).await;
        self.stop.store(true, Ordering::SeqCst);
        let mut network = tokio::task::spawn_blocking(move || self.network.join().unwrap())
            .await
            .unwrap();
        let mut timer = self.timer.await.unwrap();
        self.active.store(false, Ordering::SeqCst);
        self.server.abort();
        let _ = self.server.await;
        let window_start_ns = ns(batch_start.duration_since(self.epoch));
        let window_end_ns = ns(batch_end.duration_since(self.epoch));
        timer.mark_window(window_start_ns, window_end_ns);
        network.mark_window(window_start_ns, window_end_ns);
        ProbeReport {
            timer,
            network,
            period_ms: 10,
            cap_per_probe: PROBE_CAP,
            window_start_ns,
            window_end_ns,
        }
    }
}
fn echo(addr: SocketAddr) -> std::io::Result<()> {
    let mut socket = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    socket.write_all(&[42])?;
    let mut byte = [0];
    socket.read_exact(&mut byte)?;
    if byte != [42] {
        return Err(std::io::Error::other("invalid synthetic echo"));
    }
    Ok(())
}

#[test]
fn empty_probe_collectors_do_not_fabricate_window_coverage() {
    let report = ProbeReport {
        timer: ProbeValues::default(),
        network: ProbeValues::default(),
        period_ms: 10,
        cap_per_probe: PROBE_CAP,
        window_start_ns: 100,
        window_end_ns: 101,
    };
    assert!(
        report.covered(),
        "empty bounded collectors are valid; window coverage is separate"
    );
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["timer"]["window_status"], "NOTRUN");
    assert_eq!(json["network"]["window_status"], "NOTRUN");
}

#[test]
fn window_coverage_keeps_delayed_deadlines_and_intersecting_network_attempts() {
    let mut timer = ProbeValues::default();
    timer.push(ProbeSample {
        scheduled_ns: Some(150),
        start_ns: 300,
        end_ns: 300,
        elapsed_ns: 150,
        success: true,
        in_window: false,
    });
    timer.push(ProbeSample {
        scheduled_ns: Some(250),
        start_ns: 260,
        end_ns: 260,
        elapsed_ns: 10,
        success: true,
        in_window: false,
    });
    timer.mark_window(100, 200);
    assert_eq!(timer.in_window_samples, 1);
    assert!(timer.samples[0].in_window);
    assert!(!timer.samples[1].in_window);
    let mut network = ProbeValues::default();
    for (start_ns, end_ns) in [(50, 150), (150, 250), (200, 210), (10, 100)] {
        network.push(ProbeSample {
            scheduled_ns: None,
            start_ns,
            end_ns,
            elapsed_ns: end_ns - start_ns,
            success: true,
            in_window: false,
        });
    }
    network.mark_window(100, 200);
    assert_eq!(network.in_window_samples, 2);
    assert_eq!(network.window_status, "PASS");
    network.mark_window(300, 400);
    assert_eq!(network.in_window_samples, 0);
    assert_eq!(network.window_status, "NOTRUN");
}

#[test]
fn control_detector_rejects_lag_outside_its_window() {
    let mut timer = ProbeValues::default();
    let mut network = ProbeValues::default();
    timer.push(ProbeSample {
        scheduled_ns: Some(250_000_000),
        start_ns: 500_000_000,
        end_ns: 500_000_000,
        elapsed_ns: 250_000_000,
        success: true,
        in_window: false,
    });
    network.push(ProbeSample {
        scheduled_ns: None,
        start_ns: 250_000_000,
        end_ns: 500_000_000,
        elapsed_ns: 250_000_000,
        success: true,
        in_window: false,
    });
    timer.mark_window(0, 200_000_000);
    network.mark_window(0, 200_000_000);
    let mut report = ProbeReport {
        timer,
        network,
        period_ms: 10,
        cap_per_probe: PROBE_CAP,
        window_start_ns: 0,
        window_end_ns: 200_000_000,
    };
    assert!(!report.detects_control());
    report.timer.mark_window(200_000_000, 400_000_000);
    report.network.mark_window(200_000_000, 400_000_000);
    assert!(report.detects_control());
}

#[test]
fn draining_real_probes_does_not_turn_an_empty_window_into_coverage() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let probes = Probes::start().await;
        let boundary = Instant::now();
        let report = probes.finish(boundary, boundary).await;
        assert!(report.covered());
        assert!(!report.timer.samples.is_empty());
        // The warmup echo establishes readiness. A zero-width workload need
        // not schedule another OS-thread attempt during drain; empty is valid.
        assert_eq!(report.timer.in_window_samples, 0);
        assert_eq!(report.network.in_window_samples, 0);
        assert_eq!(report.timer.window_status, "NOTRUN");
        assert_eq!(report.network.window_status, "NOTRUN");
        assert!(!report.detects_control());
    });
}
