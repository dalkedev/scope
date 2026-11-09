# Synthetic archive responsiveness investigation

Issue: #28. Baseline: Scope 1.2.2, `0f1d91852d2d3884f472ff10e6d7e4eabae3b80e`.

## Intent and constraints

Measure the current synchronous archive path before considering a writer rewrite.
All traffic is synthetic and loopback-only. No production persistence, security,
durability, response ordering, capture limits or existing deadlines change.
The deliverable is a reviewed harness, actual Linux/Windows evidence and an
explicit bounded-writer proposal; a production rewrite is outside this increment.

## Approach

Use an ignored unit-test harness and `cfg(test)` per-engine save instrumentation.
This keeps diagnostics out of shipping binaries and avoids a public diagnostic API.
A feature-gated binary would also work but adds a shipping feature surface; an
external-only benchmark cannot separate serialization, mutex and SQL time.

The existing save statement is timed at its actual boundaries. Collect numeric
samples after dropping the connection guard, with a fixed cap and overflow count.
Record unsuccessful statements. Queue wait is NOT RUN: this implementation has
no writer queue. SQL time includes SQLite locking, triggers and autocommit; it
is neither a disk-flush count nor isolated filesystem latency.

For each profile, run current-thread and two-worker Tokio runtimes, HTTP and TLS,
three fresh-archive samples each. Each batch has 32 concurrent logical requests
sharing a reqwest client pool, the existing three rewrite/mock rules, 30-second
clients and a 60-second cooperative batch deadline. Check exact responses,
request-index coverage, unique exchange IDs, original snapshots, all ordered
rule evidence, session revisions and reopened archive evidence. Report client,
batch and shutdown elapsed time separately. Do not export rows, bodies, IDs,
URLs, certificates or filesystem paths.

A heartbeat uses monotonic absolute deadlines; an independent OS thread probes
an echo socket served on the same runtime. Both start before the batch. A parent
Python process enforces a hard child-process timeout. Fixed sample caps bound
memory, including probes. Separate positive controls intentionally block the
current-thread runtime and corrupt synthetic persisted evidence, proving the
same detectors reject known failures. Timing thresholds apply only to controls,
never normal CI or real benchmark acceptance.

Metadata records compiler/profile/workers, SQLite readback, sample bounds,
platform and observable resource limits. Unknown measurements are NOT RUN.
Hosted workflow has contents-read only, pinned existing actions, bounded jobs,
no host package or security changes, and uploads only allowlisted numeric JSON.

## Review focus

Detector blindness, probe starvation, dropped samples, timeout cleanup, profile
mismatch, ambiguous GitHub writes, accidental private diagnostics, and unsupported
causal claims. Independent design review precedes implementation; complete-source
review precedes dispatch and PR completion.
