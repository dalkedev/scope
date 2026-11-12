# Local environment check

This optional developer command inspects existing tools and runs the prototype's
checks without installing anything. Python 3.9+ and Node.js 26.13.1+ are required.
Run it from any directory; it locates this checkout from its own file location.

```sh
python3 tools/har-compare/toolchains/doctor.py
```

The default only checks the selected Node version. Use `--node /absolute/path/to/node`
when your shell has another version. JSON goes to stdout; exit 0 means all selected
checks passed, and exit 2 means a missing tool or failed check. `--output /new/file.json`
also saves a receipt and refuses to overwrite an existing file.

Run the named dependency-free checks: product tests, quality-harness tests,
six original synthetic demonstrations and thirteen sprint scenarios:

```sh
python3 tools/har-compare/toolchains/doctor.py --verify
```

This writes fresh `tools/har-compare/out/demo-*` and `out/sprint-*` output
and test-owned temporary files. It does not replay captured requests.

## Optional prepared Rust and Zig prefix

For the sprint's headless Rust coverage, explicitly select an existing artifact
prefix. The script never downloads missing tools or dependencies:

```sh
python3 tools/har-compare/toolchains/doctor.py --verify \
  --node /opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node \
  --artifact-prefix /opt/scope-lab/toolchains
```

This checks exact Rust/Cargo versions against `rust-toolchain.toml`, checks
Zig 0.17.1, then runs:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude scope-desktop --locked --offline
```

`--artifact-prefix` is optional and has no implicit machine-specific default.
The currently supported prepared layout is Linux x86-64:

```text
toolchains/
  rust-<repository-channel>-x86_64-unknown-linux-gnu/bin/
    cargo rustc rustdoc rustfmt
  zig-x86_64-linux-0.17.1/zig
  cargo-home/                 # previously fetched, reviewed dependencies
  cargo-target/               # created/updated by verification
  zig-global-cache/           # created/updated by verification
  zig-local-cache/            # created/updated by verification
```

The versioned Rust and Zig distributions are external artifacts, not vendored
in this repository. Supply verified existing installations and a populated Cargo
cache. An empty cache fails offline; the command does not repair it using the
network. A version probe establishes availability, not download authenticity.
The sprint's separate signature/hash receipts document the prepared downloads.

The small local compiler wrappers adapt cc-rs' Rust target spelling to
`x86_64-linux-gnu.2.35`; unsupported targets fail. They resolve the selected Zig
binary from a command-scoped variable, so they do not embed the sprint's path.
No system compiler, linker, root dependency lockfile or shell configuration is
replaced. Tool/caches/limited environment settings apply only to child processes;
`HOME` is preserved and application credentials are omitted from those children.

`--verify` records `Cargo.lock` hashes before and after checking and fails if they
differ. The receipt names every selected, completed and unrun check, tool availability,
missing prerequisites, check output and durations. Product-test and quality-test
counts remain separate; Rust reports its own actual passed-test count. A failing
step stops subsequent verification steps. A missing offline dependency is a failed
Rust check with a missing prerequisite, never a silent skip or passing result. Commands have finite timeouts; on POSIX,
a timeout terminates that command's process group. The default doctor creates no
cache directories, aside from an explicitly requested receipt file.

## Check the coordinator itself

```sh
python3 -B -m unittest discover -s tools/har-compare/toolchains -p 'test_*.py' -v
```

Seven doctor process tests use disposable miniature repositories and
controlled executable fixtures. They exercise missing/invalid Node, receipt
collisions, unchanged default-mode locks, separate check names, a changed protected
lock and a child dependency failure that must include `--locked --offline`.
They test this command's coordination and reporting, not the behavior of a fake
Rust compiler. No real repository lock, dependency, credential or network is used.
Nine additional native coordinator tests verify refused build directories, missing binaries, bounded event reads and private-archive cleanup after a failed or timed-out producer.
The separate preparation receipt records a real Cargo missing-cache failure;
`--verify` exercises the actual installed toolchain/cache when selected.

## Scope of the result

A passing result establishes the selected HAR checks and optional headless Rust
tests on this host. It does not establish desktop GTK/Tauri packaging, the root
smoke script, device capture, live-archive safety, remote Actions execution or
release readiness. `scripts/smoke.mjs` needs the desktop engine and currently
reserves the default demo/proxy ports, so it is intentionally excluded.

The verified recovery run counted **47** headless workspace tests and **5**
additional bridge-module tests separately. The latter are in an artifact-only
harness that includes the unchanged bridge source without GTK; they are not
included in this doctor's `rust-headless-tests` check. Its real MCP stdio smoke
is also separate. Neither additional check opens the normal application data
directory. The sprint's `research/headless-bridge/README.md` contains exact repeat
commands and receipt paths. Counts describe those receipts, not a fixed future
workspace test count.

The lab and browser commands are intentionally separate and must be requested
explicitly with their already locked dependencies present. This command does not
attempt browser installation or silently treat an unavailable sandbox as a pass.

## Optional native capture interoperability proof

This separate developer harness produces fresh synthetic HAR through the actual
`scope_core::Engine` proxy and `export_har`. It opens a new private artifact
archive, binds both the HTTP fixture and each proxy to `127.0.0.1:0`, captures
three requests in each of four sessions, then runs the dependency-free CLI.
It never opens an existing application archive or uses replay. It requires the
prepared Linux x86-64 Rust/Zig/Cargo cache described above. No downloads occur.

From the checkout root, select an existing prepared prefix and Node executable;
create a disposable parent for new build and capture artifacts:

```sh
SCOPE_NATIVE_PARENT="$(mktemp -d)"
python3 -B tools/har-compare/toolchains/build_native_capture.py \
  --work-dir "$SCOPE_NATIVE_PARENT/build" \
  --artifact-prefix /opt/scope-lab/toolchains \
  --node /opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node

python3 -B tools/har-compare/toolchains/native_capture.py \
  --binary "$SCOPE_NATIVE_PARENT/build/target/debug/native_capture" \
  --node /opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node \
  --output-parent "$SCOPE_NATIVE_PARENT"
```

The absolute prefix/Node examples identify an optional local tool directory; change
them explicitly on another host. The scripts locate this checkout from their
own location. They do not change global `PATH`, `HOME`, auth, system trust or
root dependency files. The build directory must be new and outside the checkout.
Build preparation copies the root lock to that directory and resolves this
standalone harness using `cargo metadata --offline --filter-platform
x86_64-unknown-linux-gnu`. Only that artifact copy may change. The actual build
then requires `cargo build --locked --offline`. Direct harness dependencies are
pinned; root `scope-core` dependencies retain the copied lock's resolution.
Missing cached dependencies fail with the actual diagnostic and are never fetched.

The coordinator authors these literal expectations independently of CLI output:

| Native candidate | Configuration | Exit | Expected evidence |
| --- | --- | --- | --- |
| Missing user role | Shape default | 1 | `field-removed` at `/user/role`, original `/users` entry |
| HTTP 500 | Shape default | 1 | `http-failure`, 200 → 500, original `/failure` entry |
| New timestamp | Strict + `/health` scoped ignore | 0 | One ignored JSON node; no findings |
| Same new timestamp | Strict with no ignore | 1 | `value-changed` at `/timestamp`, original `/health` entry |

Four `inspect` calls must report exactly three JSON operations each. All native
entries must be completed, unedited upstream captures without rules/errors;
body sizes and original report entry indices are checked. JSON/HTML/JUnit must
omit the synthetic query, authorization and body marker values; JUnit is parsed
with Python's standard XML parser. The fixture remains running while the first
analysis executes and must count exactly twelve requests before and after it.
That establishes no replay to this controlled upstream. It does not instrument
arbitrary network syscalls.

Next, the producer and proxies stop, and the private archive containing its CA,
private key and SQLite files is deleted. All four inventories and twelve report
files are regenerated and must match byte for byte with unchanged HAR inputs.
This proves operation after the producer stops; it does not claim an OS network
sandbox. Synthetic HAR, reports, source/build metadata and receipts are retained.
No browser rendering, HTTPS interception, device/desktop capture, production
archive safety, remote Actions or release readiness is established.

Producer events must finish with a newline within a 30-second monotonic deadline
and fit within 64 KiB. Nonblocking byte reads retain subsequent lines when a
single read receives multiple events. Incomplete, oversized or malformed events
fail without echoing their contents. Tests select a shorter internal deadline
and use real child processes to exercise partial lines, combined events, byte
limits, premature EOF and termination/archive cleanup after a timed-out producer.

Each run uses a new output directory, checks the root lock hash, records binary
and producer-source hashes, and writes `receipt.json`. Ordinary failure also
terminates the producer process group and deletes the synthetic private archive.
A force-killed coordinator can interrupt cleanup; never reuse a residual archive.
Refusing existing archives is also enforced by the Rust producer before opening
an engine. The boundary suite invoked above tests coordinator cleanup/refusal using
controlled processes; those tests are counted separately from actual native
capture evidence.

The real sprint build/run receipts live in the artifact research directory,
not this public checkout. No generated CA/key/database is a deliverable.
