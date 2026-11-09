//! Compiled only in the core unit-test binary, never in shipping applications.
use super::*;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

mod tests;

const CLIENTS: usize = 32;
const SAVE_CAP: usize = 256;

#[derive(Default, Serialize)]
struct SaveSample {
    serialization_ns: u64,
    mutex_wait_ns: u64,
    statement_ns: u64,
    serialized_bytes: usize,
    success: bool,
}
#[derive(Default, Serialize)]
pub(super) struct SaveSamples {
    samples: Vec<SaveSample>,
    dropped: usize,
}
#[derive(Default)]
pub(super) struct SaveMetrics {
    enabled: AtomicBool,
    values: Mutex<SaveSamples>,
}
impl SaveMetrics {
    pub(super) fn start(&self) -> Option<Instant> {
        self.enabled.load(Ordering::Relaxed).then(Instant::now)
    }
    pub(super) fn record(
        &self,
        start: Option<Instant>,
        serialized: Instant,
        acquired: Instant,
        finished: Instant,
        bytes: usize,
        success: bool,
    ) {
        if let Some(start) = start {
            self.push(SaveSample {
                serialization_ns: ns(serialized.duration_since(start)),
                mutex_wait_ns: ns(acquired.duration_since(serialized)),
                statement_ns: ns(finished.duration_since(acquired)),
                serialized_bytes: bytes,
                success,
            });
        }
    }
    fn push(&self, sample: SaveSample) {
        // This lock is outside the DB guard; the harness reads only after the batch.
        let mut values = self.values.lock().unwrap();
        if values.samples.len() < SAVE_CAP {
            values.samples.push(sample);
        } else {
            values.dropped += 1;
        }
    }
    fn take(&self) -> SaveSamples {
        std::mem::take(&mut *self.values.lock().unwrap())
    }
}
fn ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

fn valid_evidence(rows: &[Exchange], session: &str, rules: &[String; 3]) -> bool {
    let mut ids = BTreeSet::new();
    let mut indexes = BTreeSet::new();
    rows.len() == CLIENTS
        && rows.iter().all(|e| {
            let index = url::Url::parse(&e.url).ok().and_then(|url| {
                let values: Vec<_> = url
                    .query_pairs()
                    .filter(|(key, _)| key == "i")
                    .map(|(_, v)| v.into_owned())
                    .collect();
                (values.len() == 1)
                    .then(|| values[0].parse::<usize>().ok())
                    .flatten()
            });
            let expected = [
                ("rewriteRequest", "request"),
                ("mockResponse", "request"),
                ("rewriteResponse", "response"),
            ];
            ids.insert(&e.id)
                && index.is_some_and(|i| i < CLIENTS && indexes.insert(i))
                && e.session_id == session
                && e.method == "GET"
                && e.completed
                && e.error.is_none()
                && e.status == Some(201)
                && e.provenance == TrafficProvenance::Mock
                && e.request_body == "request rule"
                && !e.request_body_base64
                && e.response_body == "response rule"
                && !e.response_body_base64
                && e.request_headers.iter().any(|h| {
                    h.name.eq_ignore_ascii_case("x-measure-index")
                        && h.value.parse::<usize>().ok() == index
                })
                && e.original_request.as_ref().is_some_and(|v| {
                    v["body"] == ""
                        && v["method"] == "GET"
                        && v["url"] == e.url
                        && v["headers"].as_array().is_some_and(|headers| {
                            headers.iter().any(|h| {
                                h["name"] == "x-measure-index"
                                    && h["value"].as_str().and_then(|s| s.parse::<usize>().ok())
                                        == index
                            })
                        })
                })
                && e.original_response
                    .as_ref()
                    .is_some_and(|v| v["body"] == "mock body" && v["status"] == 201)
                && e.applied_rules.len() == 3
                && e.applied_rules.iter().enumerate().all(|(n, r)| {
                    r.rule_id == rules[n]
                        && r.name == format!("Rule {}", n + 1)
                        && r.action == expected[n].0
                        && r.stage == expected[n].1
                })
        })
}

mod probes;
mod workload;

// Explicitly ignored: manually launched by scripts/measure-archive.py, never a CI timing gate.
#[test]
#[ignore = "opt-in bounded synthetic measurement"]
fn archive_responsiveness() {
    let workers: usize = std::env::var("SCOPE_MEASURE_WORKERS")
        .unwrap()
        .parse()
        .unwrap();
    assert!(workers == 0 || workers == 2, "unsupported runtime");
    let samples: usize = std::env::var("SCOPE_MEASURE_SAMPLES")
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=5).contains(&samples), "unsupported sample count");
    let mut builder = if workers == 0 {
        tokio::runtime::Builder::new_current_thread()
    } else {
        let mut b = tokio::runtime::Builder::new_multi_thread();
        b.worker_threads(workers);
        b
    };
    let runtime = builder
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    let report = runtime.block_on(workload::run(samples, workers));
    let output = std::env::var_os("SCOPE_MEASURE_OUTPUT").expect("output required");
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert_eq!(
        report["status"], "PASS",
        "synthetic measurement/evidence/control failure; inspect numeric report"
    );
}
