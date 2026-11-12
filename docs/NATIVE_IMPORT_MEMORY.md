# Native archive import memory

`Engine::import_session(Value)` now takes the owned exchange tree out of its
input before deserialization. Previously it cloned every exchange, including
body strings and originals. Validation still finishes before the existing atomic
SQLite transaction, and the public API and import format are unchanged.

This is a measured, bounded improvement toward [issue #12](https://github.com/dalkedev/snurpo/issues/12).
Bodies remain limited to 16 MiB each; native imports still materialize their
input and validated exchanges. Disk-backed bodies, streaming/backpressure and
long-session scalability remain future work.

## Recorded result

Date: 2026-11-04. Linux x86_64, Rust 1.99.1, Zig 0.17.1 native linker, unchanged
Cargo.lock, **debug/unoptimized with debuginfo**. Three fresh-process samples per
cell per implementation, 24 accepted samples total. Baseline production source:
`5179f4d1329f041c9aa58c2e89cf3e1260e1b97e`. The same frozen benchmark source was
used for baseline and candidate; source hashes and every sample are in the
[path-free measurement record](benchmarks/native-import-2026-11-04.json).

Each exchange has both a request and response body of the listed size. The large
cell therefore has 64 MiB of decoded bodies. Binary bodies use base64 in the
native input; their serialized input is larger than the decoded corpus.

| Body kind | Exchanges | MiB per body | Baseline peak KiB, median [range] | Candidate peak KiB, median [range] | Median reduction |
| --- | ---: | ---: | ---: | ---: | ---: |
| utf8 | 1 | 1 | 32052 [32036–32116] | 29996 [29808–30028] | 6.41% |
| utf8 | 4 | 8 | 207916 [207884–207980] | 142812 [142536–142888] | 31.31% |
| binary | 1 | 1 | 35108 [35068–35120] | 32320 [32296–32560] | 7.94% |
| binary | 4 | 8 | 267944 [267904–268016] | 180552 [180544–180600] | 32.62% |

For the large cells, the median absolute peak reduction is 63.58 MiB (UTF-8) and
85.34 MiB (binary). The regression test separately observed two body-sized Rust
allocations on baseline versus one on candidate for an owned 1,048,583-byte body;
the remaining allocation validates the decoded body. That test measures one
allocation size on its calling thread, rather than total live memory.

There is **no speedup claim**. Import-time medians/ranges are retained in the raw
record. Small UTF-8/binary medians increased 0.18%/1.67%; large medians decreased
0.49%/3.04%. These three-sample debug timings are not a dedicated speed study.
This result does not establish release-build, other-platform or capture/replay
performance. Release-profile measurements were **NOT RUN**.

## What is measured and checked

The benchmark calls the real public import API with a newly generated synthetic
native `Value`. A streaming counting writer records serialized input size without
materializing another full JSON string. Its timer covers only `import_session`.
Cargo, archive setup, fixture generation, byte counting, readback and reopen are
outside that timer.

Memory is absolute Linux `VmHWM`, sampled immediately after import and before
readback/reopen controls. It includes process startup and fixture generation; it
is not an incremental allocation count or a reset import-only peak. Linux
[documents VmHWM and approximate RSS accounting](https://docs.kernel.org/filesystems/proc.html#process-specific-subdirectories).
Allocator retention and system conditions can affect these process estimates.
SQLite/WAL/SHM lengths are reported separately from memory and before/after close.

Every accepted sample checks decoded request/response bytes, repeated header
order, pairing, originals, normalized metadata, regenerated identities, completion
handling and reopened persistence. It removes its disposable archive, including
the synthetic CA, before emitting a successful JSON result. Ordinary errors use
TempDir cleanup; forced termination can interrupt cleanup, so never reuse a
residual benchmark archive. No real archive, network traffic, browser or profile
is used. JSON output contains no generated path or private key.

Five focused tests cover the allocation regression, paired binary/plain records
and originals after reopen, malformed envelopes, late-row atomic rejection and
exact decoded binary limit acceptance. Rejection cases include malformed base64
and 16 MiB + 1 byte in either direction, with session/audit/export and database
row-count controls. Capture and device code are unchanged.

## Reproduce

Use a clean checkout with the pinned Rust toolchain, ordinary native build
prerequisites and Linux `/proc`. Build once, then run the binary directly so
Cargo is excluded from every sample:

```sh
set -eu
cargo build -p scope-core --example import_memory --locked
run_samples() {
  for mode in utf8 binary; do
    for count in 1 4; do
      mib=1
      if [ "$count" = 4 ]; then mib=8; fi
      for repeat in 1 2 3; do
        "$1" "$mode" "$count" "$mib"
      done
    done
  done
}
run_samples target/debug/examples/import_memory > candidate.jsonl
cargo test -p scope-core --test native_import_memory --locked
```

For the baseline, create a separate checkout at the baseline commit and copy
only the benchmark source into it. In the same shell (retaining `run_samples`),
build with the same toolchain, linker, profile and lock, then run fresh samples:

```sh
set -eu
git worktree add --detach ../scope-import-before 5179f4d1329f041c9aa58c2e89cf3e1260e1b97e
mkdir -p ../scope-import-before/crates/scope-core/examples
cp crates/scope-core/examples/import_memory.rs ../scope-import-before/crates/scope-core/examples/
(cd ../scope-import-before && cargo build -p scope-core --example import_memory --locked)
run_samples ../scope-import-before/target/debug/examples/import_memory > baseline.jsonl
```

The benchmark accepts only the two bounded cells shown above and requires Linux
memory counters. Keep baseline/candidate sample sets separate, retain failed
runs as failures, and compare medians plus ranges. The recorded exploratory
fixture that cloned rows during setup was discarded before freezing this final
harness; those earlier samples are excluded from the published record.
