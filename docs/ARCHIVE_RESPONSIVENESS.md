# Archive-write responsiveness investigation

This investigates [#28](https://github.com/dalkedev/scope/issues/28) against
Scope 1.2.2. It does not change production scheduling, SQLite settings, capture
limits, response ordering, native security settings or existing test deadlines.
Six `Engine::save` calls per successful mock exchange mean 192 upserts for 32
requests. That is **not** a count of disk flushes. The corrected hosted Linux and
Windows dataset below passes all 48 synthetic workload batches and every required
positive control; it does not establish a production performance improvement.

## Reproduce

Use the pinned Rust 1.99.1 toolchain, Python 3.12 and the locked dependencies:

```sh
python scripts/measure-archive.py --profiles debug release --samples 3 --workers 0 2 --output-dir .runtime/archive-measurement
```

The ignored unit-test harness and all observers are compiled only with
`cfg(test)`; no diagnostic API or collector ships in the application. Normal
CI runs deterministic detector regressions, not the ignored timing workload.
The dedicated workflow supports manual dispatch and reuse. Before that new file
is registered on the default branch, the existing HAR composite workflow provides
an explicit `archive-measurement` manual choice and calls it locally. Dispatch
the registered entry point against the reviewed branch:

```sh
gh workflow run har-compare-action.yml --ref YOUR_BRANCH -f diagnostic=archive-measurement
```

Only dedicated measurement jobs run for this explicit choice; the HAR consumer
is skipped. Default HAR dispatch and regular push/PR checks keep their existing
commands. The checks/release workflows are unchanged. Dispatch has a separate concurrency group,
contents-read permission, existing commit-pinned actions, no system-package installation
or native-security changes, and a 45-minute job limit. Each runner exports one
allowlisted numeric JSON artifact; compiler logs, archives and temporary CA keys
are excluded. The Python runner has an overall 30-minute budget, bounded builds
and child processes, and kills/reaps children on hard timeout. A failure to
establish process-tree cleanup ends further sampling. A Python cancellation or
exception, including POSIX SIGINT/KeyboardInterrupt, also invokes the same bounded
tree termination/reaping before the original exception is reraised; uncertain
cleanup is attached as a fixed note. Abrupt parent termination is outside this
cleanup guarantee; native Windows console Ctrl+C coverage is NOT RUN.

## Workload and interpretation

Each runtime/profile combination runs three independent HTTP and three TLS
batches, each using a fresh temporary file-backed archive and client pool.
Thirty-two barrier-released logical requests use shared reqwest client clones,
HTTP/1.1, a 30-second client deadline and 60-second cooperative batch deadline.
These are logical clients, not a claim of 32 simultaneous network connections.
In JSON, workers=0 identifies current-thread execution on the calling thread;
workers=2 selects two Tokio worker threads. Both runtimes cap the blocking pool
at two threads and use one separate OS network-probe thread.
URLs target only loopback; three real rules rewrite the empty request, mock a
201 response, and rewrite that response. TLS uses actual CONNECT interception
and the newly generated CA. Nothing is sent to an external origin.

Client status/body, exact request-index coverage, unique exchange IDs, original
request URL/method/body/pairing header, original response status/body, all ordered
rule IDs/names/actions/stages, completion/error/provenance, session revision
increments and reopened archive evidence are checked. A batch must produce 32
rows and 192 successful save samples/revision increments, without overflow.
Independent controls corrupt rule evidence, restore/revalidate the baseline,
then delete a row; the same detector must reject both corrupted archives.

Save samples time actual serialization, connection-mutex acquisition and the
existing SQLite statement. Samples are collected **after** releasing the DB
guard, at most 256 per batch, with overflow accounting. SQL elapsed includes
SQLite internal locks, triggers, checkpoint/autocommit work and OS effects;
it does not separate fsync, storage service time or SQLite busy-handler wait.
Queue wait is NOT RUN because there is no writer queue. Summed per-save durations
may overlap between workers and must not be equated to batch wall time.
Instrumentation has overhead; this is an observed test build, not an unobserved
production benchmark.

A 10ms absolute-deadline heartbeat runs on the measured Tokio runtime. A separate
OS thread connects to a loopback echo socket served by that runtime, with finite
connect/read/write deadlines. Probes are ready before load. The report records
scheduled/start/end times, skipped timer periods, failures and exact workload
window membership; use only `in_window` samples for workload responsiveness.
Drain observations remain distinguishable. Sub-period batches can have no
in-window sample and explicitly report NOTRUN. No timing threshold gates normal
batches. A separate current-thread control intentionally blocks for 300ms and
requires both timer and network detectors to reject that interval using a 100ms
threshold. Two-worker control is NOT RUN, because another worker could progress.
Probe collectors cap at 8192 entries each; overflow invalidates results.

Setup (including CA/session/rule creation), probe drain, proxy shutdown and
archive validation are outside batch timing. Shutdown time is reported separately.
Batch timing starts before spawning/releasing clients. Cooperative timeout flags
and observed wall-budget violations are both reported; only the external parent
can bound a runtime blocked in synchronous code.

Metadata records commit/dirty state, compiler, OS/architecture, profile, runtime
worker count, build concurrency, SQLite version/compile options and actual
journal/synchronous/busy-timeout/page/cache/checkpoint settings. Logical CPU,
affinity and cgroup limits are recorded where exposed. Unknown memory/I/O quotas,
filesystem type and peak RSS are NOT RUN, not assumed unlimited. User Cargo
configuration is not inspected; clean hosted runs are the controlled profile
reference. Do not compare debug and release or differently constrained hosts as
if they were matched experiments.

## Corrected hosted results and provenance

**PASS:** [run 42293997874](https://github.com/dalkedev/scope/actions/runs/42293997874),
measured source `19aec4fc8ca63551c05a0380c157c5593f048efa`, clean checkouts on
`ubuntu-22.04` and `windows-2022`, Rust 1.99.1, Python 3.12.16 on Linux /
3.12.10 on Windows, four logical CPUs and two Cargo build jobs on each runner.
Linux affinity exposed four CPUs;
Windows affinity, memory/I/O quotas, filesystem type and peak RSS are NOT RUN.
Unknown cgroup limits remain null, not assumed unlimited. Linux image version is
`20261101.347.1`; Windows image version is NOT RUN (not captured in the report).
Its selected label is `windows-2022`.
Exact per-platform metadata and SQLite compile options remain in the raw datasets.

All **48/48** workload batches passed: three samples for each OS × profile ×
runtime × transport combination, 1,536 correct completed clients and persisted
rows, and 9,216 successful measured save statements/revision increments. Each
batch independently verified all rule evidence, original snapshots, request
pairing and reopened IDs. All 48 rule-corruption and 48 independent row-loss
controls detected the deliberate corruption, with baseline restoration checked.
The four current-thread timer/network blocking controls passed (debug/release on
both platforms); two-worker blocking controls remain intentionally NOT RUN.
No sample overflow, probe failures in workload windows, cooperative timeout or
observed client/batch deadline violation occurred.

Both platforms reported SQLite 3.51.1, WAL, synchronous=2 (FULL), busy_timeout=5000ms,
page_size=4096, cache_size=-2000 and wal_autocheckpoint=1000. The harness did not
change these values. Every batch has both timer and network window coverage;
raw reports retain all sample timestamps and explicit coverage states.

- [Linux raw numeric report](benchmarks/archive-responsiveness-2026-11-06-linux.json)
- [Windows raw numeric report](benchmarks/archive-responsiveness-2026-11-06-windows.json)
- [Derived distributions, source hashes and artifact receipts](benchmarks/archive-responsiveness-2026-11-06-summary.json)

The downloadable ZIP SHA-256 values were matched to GitHub's API
artifact digests before extracting the sole allowlisted `measurement.json` file.
The summary records artifact IDs/digests and raw JSON SHA-256 values. All runtime
reports passed the strict schema validator, source/clean-tree/run/attempt/sample
identity checks and exact evidence counts. Independent review recalculated all
16 groups and every displayed table cell. Recorded whole-file hashes identify
the measured commit. Subsequent changes add documentation/results and correct
one ordinary-CI assertion in the probe regression tests, described below. The later
runner cancellation fix and Python-only test workflow are recorded as explicit
source exceptions in the summary. The merge of Scope 1.3.0 is recorded separately:
it changes engine/rule code and release metadata, so the hosted values above remain
historical Scope 1.2.2 observations, not current-head performance evidence.
Through that merge, the diagnostic workload and probe runtime implementation
remain unchanged; the probe file is byte-identical before its first `#[test]`
function. The summary
defines that UTF-8 prefix boundary precisely and records its verified SHA-256.
No diagnostic timing logic, threshold or window filter changed after the measured
commit. A current-head smoke run checks compatibility without replacing the
historical comparison.

The later [target-bound validation correction](#target-bound-validation-follow-up--2026-11-09)
changes the detector and its workload call sites after measurement. It does not
replace the historical hashes or establish that the new check ran in 2026-11-06
measurements.

### Descriptive timings (milliseconds)

Each row has three batches. Batch is median (min–max). Client p95 pools the 96
client durations within that row using nearest rank. Phase columns are medians
of each batch's **sum of 192 saves**. Timer and echo columns are maxima of samples
whose deadlines/attempts cover the workload window. These are small descriptive
samples, without confidence intervals or significance claims. Do not pool profiles,
add overlapping mutex/statement sums, or treat OS/hardware as a controlled single
variable. Raw/derived files retain every sample, probe count and shutdown duration.

| OS | Profile | Tokio workers | Transport | Batch median (range) | Client p95 | Σ serialize | Σ mutex wait | Σ SQL | Max timer lag | Max echo RTT |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Linux | debug | current | HTTP | 121.56 (110.79–123.57) | 121.12 | 8.48 | 0.04 | 89.76 | 111.69 | 111.41 |
| Linux | debug | current | TLS | 198.53 (196.86–225.39) | 201.41 | 8.63 | 0.05 | 82.26 | 96.63 | 85.47 |
| Linux | debug | 2 | HTTP | 85.19 (84.76–87.92) | 85.55 | 9.34 | 56.22 | 75.20 | 45.54 | 74.45 |
| Linux | debug | 2 | TLS | 146.34 (140.94–193.33) | 180.68 | 9.09 | 50.86 | 79.14 | 62.20 | 53.21 |
| Linux | release | current | HTTP | 74.94 (69.67–82.82) | 82.27 | 0.60 | 0.01 | 68.32 | 72.61 | 74.38 |
| Linux | release | current | TLS | 131.55 (121.76–157.09) | 125.68 | 0.62 | 0.01 | 68.54 | 71.97 | 76.54 |
| Linux | release | 2 | HTTP | 72.54 (71.52–75.02) | 73.88 | 0.77 | 65.22 | 68.05 | 48.92 | 63.89 |
| Linux | release | 2 | TLS | 118.56 (116.03–153.06) | 143.52 | 0.70 | 60.80 | 69.67 | 54.57 | 35.75 |
| Windows | debug | current | HTTP | 673.30 (651.91–753.71) | 750.43 | 8.97 | 0.04 | 636.30 | 731.97 | 731.12 |
| Windows | debug | current | TLS | 749.69 (725.40–851.98) | 849.28 | 8.75 | 0.04 | 613.30 | 670.23 | 666.42 |
| Windows | debug | 2 | HTTP | 648.30 (643.08–721.82) | 717.12 | 9.81 | 606.98 | 637.76 | 407.40 | 629.28 |
| Windows | debug | 2 | TLS | 717.35 (675.94–723.68) | 720.50 | 9.56 | 562.43 | 623.42 | 580.99 | 533.91 |
| Windows | release | current | HTTP | 600.55 (575.29–606.54) | 605.63 | 1.00 | 0.01 | 586.73 | 596.11 | 595.68 |
| Windows | release | current | TLS | 599.64 (573.69–691.05) | 689.65 | 1.01 | 0.01 | 559.26 | 568.55 | 542.86 |
| Windows | release | 2 | HTTP | 563.19 (561.49–573.37) | 569.16 | 1.10 | 545.89 | 555.80 | 559.00 | 562.33 |
| Windows | release | 2 | TLS | 578.15 (570.08–624.50) | 583.99 | 1.11 | 534.21 | 561.51 | 409.62 | 416.58 |

The current-thread profiles show delayed timer wakes and loopback echo servicing
during these bursts. In these samples the synchronous SQL phase occupies much
of batch wall time; two-worker runs also show connection-mutex waiting. These
observations justify a controlled blocking-writer experiment, not a claim that
SQLite disk latency caused the earlier Windows timeouts. SQL time includes
internal locks, triggers, autocommit/checkpoint work and OS effects. TLS adds
other scheduling/cryptographic work; no isolated storage or zero-write baseline
was measured. The historical 29-client timeout failure was **not reproduced**.
The four intentional blocking controls validate detection of known executor
stalls; they do not attribute real-workload lag to one underlying cause.

No production rewrite is implemented. The next design should first isolate
scheduling while preserving every persistence barrier, then measure it against
this harness with matched profiles. The size/admission/finalization prerequisites
below remain unresolved; batching is a separate later experiment.

## Preserved control-startup regression

The first complete [measurement run 42292352293](https://github.com/dalkedev/scope/actions/runs/42292352293)
at `04c78d202d61689a4c9444da120c375e0818a4fb` passed Linux. Windows passed all
24 workload batches (32 correct clients/rows and 192 successful saves/revision
increments each), but its debug and release current-thread blocking controls
failed. This is a **FAIL of harness validation**, not archive evidence loss.

Windows network readiness took approximately 24–26ms; the timer had already been
armed with its first deadline at approximately 11ms. After the intentional 300ms
block, that overdue deadline correctly fell outside the control window. The
network detector observed the block, while timer coverage was NOT RUN. The
[bounded numeric regression receipt](benchmarks/archive-control-startup-2026-11-06.json)
retains source/run/artifact provenance and both failed controls. No raw job logs,
archive rows or host identities are included.

The harness now waits for independent network readiness **before** arming the
heartbeat. Window filtering, thresholds, workload and production behavior remain
unchanged. This fixes the observed readiness race; it does not promise
deterministic OS scheduling. The corrected-source hosted dataset above passes
the unchanged controls on both platforms; the original failed report remains preserved for regression evidence.

Local debug smoke: all four HTTP/TLS × current-thread/two-worker batches passed
with one sample per case, including the blocking and persisted-evidence controls.
This is a harness smoke check, not comparative performance evidence.

GUI/Android, real captures, external endpoints, sustained/large-body workloads,
fsync counts, isolated storage latency and production performance: **NOT RUN**.

## Zero-width regression test correction

The [macOS job on the first results commit](https://github.com/dalkedev/scope/actions/runs/42295532143/job/128159662751)
failed only `draining_real_probes_does_not_turn_an_empty_window_into_coverage`:
its assertion required an active network sample during approximately 20ms of
drain even though the workload window has zero width. `Probes::start` already
proves a successful warmup echo. The OS thread may observe the stop flag before
its next active attempt, so an empty collector is valid and must remain NOT RUN.

Only that incidental nonempty-network assertion was removed. Real socket
readiness, timer observation, both zero in-window counts, both NOTRUN states and
false-control rejection remain checked. Deterministic interval/empty-collector
regressions and the four hosted blocking controls remain unchanged. This is a
normal-CI test correction, not a measurement change or timeout increase. The
summary preserves the failed commit/job and the unchanged-runtime-prefix proof.

## Parent cancellation cleanup follow-up

The original runner cleaned detached child trees only when its deadline expired.
A parent SIGINT/KeyboardInterrupt could leave Cargo or a test process running.
The runner now shares finite tree termination and direct-child reaping between
TimeoutExpired and parent BaseException paths, then reraises the original parent
exception. Windows taskkill failure/timeout still marks cleanup uncertainty;
direct-child fallback does not falsely establish descendant cleanup. The helper
uses at most 10 seconds for taskkill and 10 seconds for reaping per attempt.

Disposable-process regressions launch a sleeping child and descendant, prove
actual POSIX SIGINT cleanup and preserved exception propagation, and always clean
the fixture's own recorded PIDs/groups. Native Windows tests use injected Python
KeyboardInterrupt/RuntimeError with real Windows process trees; console Ctrl+C
coverage is NOT RUN. A manually selected `runner-regressions` choice in the HAR
entry point runs only Python tests on Linux/Windows, with a five-minute job limit:

```sh
gh workflow run har-compare-action.yml --ref YOUR_BRANCH -f diagnostic=runner-regressions
```

Rust setup/builds, measurements and report upload are skipped for this choice.
The successful-process output/status contract and Rust measurement implementation
are unchanged. Existing Linux/Windows results remain pinned to measured source
`19aec4f`; no long benchmark rerun is implied by this cleanup-only correction.
Exact follow-up test receipts and head are recorded in the PR description.

## Scope 1.3.0 compatibility revalidation — 2026-11-09

Merged source `5c8f4aed5f96361289629ca4ed2bc03a66a90287` includes released
Scope 1.3.0 from `cb5fccd3253a1a3b3ccb0ff4efe83350a94518e8`. Its clean-tree
macOS arm64 compatibility check used Rust 1.99.1 and local Python 3.14.6;
the hosted workflow still selects Python 3.12.

- **PASS:** 78 locked headless Rust tests (53 core, 23 device, two MCP), including
  real HTTP/TLS rule ordering, endpoint pause edits, persistence/reopen, concurrency
  and the diagnostic detector regressions. The ignored timing workload ran separately.
  Linux-specific GLib tests have zero cases on macOS and are not claimed here.
- **PASS:** all 33 Python script tests, including the 16 measurement-runner tests
  with real POSIX SIGINT and child/descendant cleanup; formatting and exact GLib
  archive/patch/locked-selection verification.
- **PASS:** one bounded debug sample for each HTTP/TLS × current-thread/two-worker
  combination: four batches, 128 successful clients and reopened rows, 768 successful
  save samples, no overflow or deadline violation, and certain child cleanup.
  All corruption/loss controls detected their changes and restored baselines passed.
  Both probes covered every workload window; the current-thread blocking control
  passed and its two-worker counterpart remains intentionally NOT RUN.
- **PASS:** both historical raw report hashes and schemas, source hashes, the
  unchanged probe-runtime prefix, and all numeric fields in 16 derived groups
  were independently rechecked after the merge.

The [numeric compatibility report](benchmarks/archive-compatibility-2026-11-09-macos.json)
records that exact source and clean-tree state. It is a small compatibility smoke
check, not a new cross-platform or release-profile performance comparison. The
48 hosted Linux/Windows batches remain pinned to their original Scope 1.2.2 source.
GUI, physical Android, current-head hosted measurements and release-profile timing
were **NOT RUN** by this revalidation. No production writer change is included.

## Target-bound validation follow-up — 2026-11-09

Independent review of release source `9789d92013ef2aac64941ebaca18a0ce691a6d4b`
found that the detector checked the request index and equality between captured
and original URLs, but did not bind either URL to the target actually sent.
Changing both stored URLs to `http://127.0.0.2:9/unrelated?i=0` was accepted while
changing only the original URL was rejected. This is a reproducible acceptance
gap, not evidence of a production capture defect or historical archive corruption.

The correction at `ad598d00a027e99ffffed79041c42c881fcdc141` passes the batch's
HTTP/TLS selection into every initial, reopened, corruption-control, restored
baseline and row-loss validation. Each URL must equal the fixture's exact
`{http|https}://localhost:1/pause?i={index}` target. Original snapshot equality
then binds both stored URLs to that independently known target. Deterministic
regressions reject paired host/port/path/scheme/query/fragment changes and
cross-transport batches; valid HTTP and TLS batches remain accepted. The paired
wrong-target regression was observed failing before the correction and passing
after it.

This changes only `cfg(test)` diagnostics and their tests. The workload's actual
request construction, timed interval, save/probe instrumentation, thresholds,
report schema and production persistence remain unchanged. The historical raw
datasets and summary are byte-identical; their canonical hashes still identify
their recorded source commits, rather than these corrected detector files.

Fresh Linux x86_64 acceptance at that clean code commit used Rust 1.99.1 and
Python 3.14.4:

- **PASS:** 100 locked headless core/device/MCP tests; all 33 script tests,
  including exact GLib source/patch/locked-selection verification; formatting.
- **PASS:** one bounded debug sample per HTTP/TLS × current-thread/two-worker
  combination: four batches, 128 successful joined clients and reopened rows,
  768 successful save samples/revision increments, complete target-bound evidence,
  corruption/loss detection and baseline restoration, no overflow/deadline violation
  and no process-cleanup uncertainty. The current-thread blocking control passed;
  the two-worker blocking control remains intentionally **NOT RUN**.
- **NOT RUN here:** release-profile/comparative timing, native Windows/macOS,
  interactive desktop and Android. The separate Linux GLib regression build
  stopped at the missing `pkg-config` prerequisite, before executing its tests.

Reproduce the small compatibility gate without rerunning the historical matrix:

```sh
python scripts/measure-archive.py --profiles debug --samples 1 --workers 0 2 \
  --output-dir /tmp/scope-target-validation
```

These are compatibility/acceptance results, not new performance measurements or
a throughput/fsync improvement claim.

## Bounded writer proposal (not implemented)

Start with scheduling isolation, not transaction batching: one blocking writer
owns the SQLite connection, retains existing WAL/synchronous/busy settings and
executes each accepted save exactly once in order. An acknowledged command is a
persistence barrier, including saves before breakpoint inspection and the final
save before returning a successful response. Read operations requiring the
latest evidence must wait for their preceding barrier; rule snapshots remain
atomic per request. Preserve revision-trigger counts and original snapshots.

A candidate queue has eight command slots, at most one outstanding command per
exchange, and a 128MiB byte budget covering queued **and active** serialized
payloads. Acquire capacity before constructing another snapshot buffer. A full
queue applies bounded waiting to the producer, with an explicit cancellation or
overload result; no dropped writes, successful response or unbounded task-per-save
fallback is permitted. Reserve finalization capacity before admitting a capture
so overload cannot deadlock its terminal error write. Resource reservations are
released only after acknowledgement/finalization, not when a waiting future drops.

Also bound capture owners and admission waiters separately: a candidate envelope
is 32 admitted captures and at most eight pending admissions, with finite waiting
deadlines. Reject excess admission explicitly before forwarding or buffering a
body; do not create an unbounded task list waiting for queue capacity. Transport
connections/handshakes need their own finite limits. These limits and rejection
semantics require protocol/error-finalization tests before implementation; queue
slots alone do not bound the whole capture process.

This is a proposal, not a claim that eight slots or 128MiB is optimal. The
existing 16MiB body limit does not bound an entire JSON exchange with headers,
encoding and original snapshots. Before implementation, establish an exact
maximum encoded command size. If valid records exceed the byte budget, use a
bounded streaming/spooled representation (coordinated with #12), or reject the
design; silently reducing accepted payloads or claiming a complete memory bound
from queue length alone is unacceptable. Existing capture memory is a separate
budget and must also be measured. No production switch is justified until this
prerequisite and overload/finalization tests are satisfied.

Cancellation before enqueue creates no accepted command. After enqueue, ownership
passes to the writer; cancellation must still await/record finalization. Stop
closes admission for the affected capture generation, cancels its network work,
drains accepted writes and terminal error records, then acknowledges shutdown.
Replay ownership stays associated with its original session/generation. Never
allow a late write to target a replacement capture. Restart continues the current
interrupted-capture recovery contract; test crashes before/after final commit and
acknowledgement, including committed-but-unacknowledged commands.

On serialization/SQLite/full-disk/lock failures, propagate a specific failure to
every affected producer. Never return successful traffic without required final
evidence. Terminal persistence failure must surface as such, not be presented as
an archived error. Retain the existing finite busy timeout. Do not automatically
retry a commit with uncertain outcome. Verify rollback/autocommit state and stop
admission when storage state cannot be established.

Only after scheduling isolation is measured should a second experiment batch
independent exchanges (for example up to eight commands or 2ms). Preserve per-
exchange ordering; acknowledge every barrier only after transaction commit. No
coalescing across breakpoint/finalization barriers, missing revision increments,
or response-before-commit is allowed. Rollback fails all commands in the group.
Acceptance must include stalled/locked storage, saturation, cancelled/replayed
captures, Stop races, crash/reopen and error-finalization tests plus matched
profiles. This increment deliberately supplies measurement and design, not that
unproven production change. Issue #28 remains open.
