# Vendored GLib 0.18.5: CodeQL triage

At the initial triage below, the seven Rust alerts introduced into analysis by
PR #27 were proposed **false positives for the exact reported flows**, subject to
independent operator review. None had been dismissed at that point. The final
operator decisions are recorded separately below. No vendor exclusion or blanket
suppression is used. These classifications do not establish that every unsafe
GLib path is safe.
The vendored crate still differs from its pristine archive only by the two-line
upstream `VariantStrIter` backport; its complete byte verification is unchanged.

## Evidence boundary

Reviewed source: `079938568a97ef2be144309005b54f18e3072e8a`.
Exact Rust SARIF: [analysis 2114338864](https://api.github.com/repos/dalkedev/scope/code-scanning/analyses/2114338864),
commit `0799385`, ref `refs/pull/27/head`. Both reported rules have security severity
7.5 and precision `high`. The query source is pinned by that SARIF to CodeQL commit
`1be292a1e54696ca543b4ad0f9b4537b9687caa7`. All seven alerts were OPEN at triage.
GitHub may require repository security access to view the analysis/alert links.

| Alert | Rule | Exact primary location | Proposed operator disposition |
| --- | --- | --- | --- |
| [4](https://github.com/dalkedev/scope/security/code-scanning/4) | `rust/cleartext-logging` | `vendor/glib/src/log.rs:85` | False positive: UUID validation's boolean cannot reach the distinct log-level conversion. |
| [5](https://github.com/dalkedev/scope/security/code-scanning/5) | `rust/access-invalid-pointer` | `vendor/glib/src/object.rs:470` | False positive: unrelated C string/error pointers cannot instantiate the `GObject*` transfer-none conversion. |
| [6](https://github.com/dalkedev/scope/security/code-scanning/6) | `rust/access-invalid-pointer` | `vendor/glib/src/object.rs:493` | False positive: the same generic-type conflation at the `GObject*` full-transfer conversion. |
| [7](https://github.com/dalkedev/scope/security/code-scanning/7) | `rust/access-invalid-pointer` | `vendor/glib/src/subclass/boxed.rs:76` | False positive: the test-only derive accepts `MyBoxed*`, not string/error pointers. |
| [8](https://github.com/dalkedev/scope/security/code-scanning/8) | `rust/access-invalid-pointer` | `vendor/glib/src/value.rs:462` | False positive: the reported pointers are not the concrete initialized `GValue*` source of `copy_into_value`. |
| [9](https://github.com/dalkedev/scope/security/code-scanning/9) | `rust/access-invalid-pointer` | `vendor/glib/src/value.rs:470` | False positive: the reported pointers cannot become the concrete `GValue*` being cleared. |
| [10](https://github.com/dalkedev/scope/security/code-scanning/10) | `rust/access-invalid-pointer` | `vendor/glib/src/collections/ptr_slice.rs:900` | False positive: dropping the stored wrapper does not free its separately owned slot allocation. |

## Alert 4: UUID validation does not log a UUID

The sole complete path starts at `auto/functions.rs:857`, passes through the
generic `from_glib` helper (`translate.rs:1413–1414`), and ends in the numeric
`Unknown log level` panic (`log.rs:71–85`).

The [C UUID API](https://docs.gtk.org/glib/func.uuid_string_is_valid.html) returns
`gboolean`, not the UUID string. The binding's return type is `bool`, selecting
`FromGlib<gboolean> for bool` (`translate.rs:1417–1421`), which compares the integer
with `GFALSE`. `LogLevel` instead implements `FromGlib<u32>`; its panic formats only
that integer's value. Rust's concrete generic dispatch does not send an `i32`
boolean conversion into the `u32` log-level implementation. No UUID bytes appear
in the return value, and no logging or panic call exists on this validation path.

`codeql_logging.rs` exercises valid, malformed and empty UUID inputs as typed
booleans. A separate test converts the six known logging flags and catches the
zero-flag panic, verifying that its payload is exactly `Unknown log level: 0`.
These tests corroborate the concrete dispatch; they do not assert that arbitrary
application logging is safe.

## Alerts 5–9: generic instantiations and C output parameters

Each of these five results contains four complete paths. For alerts 5, 7 and 8
they start at `auto/functions.rs:344` (console charset), `197` (file-write error),
`224` (full file-write error), and `276` (URI hostname). Alerts 6 and 9 instead
start at `197`, `224`, `276` and `277` (URI error), excluding borrowed charset.
The reports also enumerate
60 null-initialization sources for alerts 5, 7 and 8, and 59 for alerts 6 and 9.
All enumerated source groups were inspected, not just the four displayed paths:

- Charset: `*const c_char` passed as writable output to `g_get_console_charset`.
- File, shell/spawn, key-file, markup, URI and encoding conversions: `GError*`
  passed by mutable reference; conversion to `Error` follows the non-NULL error
  branch or the documented failed-result branch.
- URI hostname and split components: allocated character-pointer outputs;
  optional NULL outputs become `None`, while required strings are converted only
  after successful parsing. Successful non-string results keep their declared
  scalar, array, path, URI or byte types.

The generic helpers at `translate.rs:1621–1631` dispatch on both pointer type `P`
and result type `T`. Charset/URI output conversions select `GString`,
`Option<GString>` or the relevant path type; `GError*` conversions select `Error`.
These inputs cannot select `ObjectRef`, `Value` or the test `MyBoxed` conversion
without an additional explicit unsafe cast absent from the reported callers.
Some displayed error paths even traverse `String`'s full-transfer implementation
(`translate.rs:1688–1689`) before reaching another unrelated sink, illustrating
that the rendered path has conflated incompatible generic instantiations.

The null initializations are also not proof that C leaves the values NULL.
The [console charset contract](https://docs.gtk.org/glib/func.get_console_charset.html)
sets a borrowed string output. The [file-writing contract](https://docs.gtk.org/glib/func.file_set_contents.html)
requires an initially NULL error pointer, leaves it NULL on success, and returns
an allocated `GError` on failure. The Rust wrapper checks the error pointer before
converting it. Optional URI outputs legitimately remain NULL and are handled by
the `Option` conversion. Treating these C calls as having no output effects loses
both the documented writes and the wrapper's branch/type constraints.

For alerts 5/6 specifically, `ObjectRef` accepts a live `GObject*` and obtains or
owns the corresponding reference. For alerts 8/9, `GValue` copy/clear operates on
initialized inline or allocated `GValue` storage. Allocation precedes copy;
`free_value` clears before freeing. For alert 7, the derive is inside `#[cfg(test)]`
and emits conversions for `*const MyBoxed` / `*mut MyBoxed`. Boxed `Value` type
erasure is protected by the registered concrete `GType`; its copy/free callbacks
allocate `Box<T>` / reconstruct that same allocation, and typed extraction checks
the type. None of these contracts permits treating the listed C string/error
outputs as these unrelated objects. Deliberately invalid unsafe callers would
violate their preconditions and are not the reported safe-wrapper flow.

`codeql_translation.rs` tests actual C charset output, file-write success and
owned-error failure, optional hostname, absent/empty/populated URI components,
key-file/shell/encoding errors, concrete object lifetime and `GValue` copy/free.
`codeql_ownership.rs` checks boxed mutable/const transfer-none clones, borrow/full
ownership and independently allocated boxed `Value` copies with clone/drop counts.
The test-only dependency enables `v2_66` for these exact APIs, below Ubuntu 22.04's
GLib version and the application's existing GTK graph's GLib requirement; it does
not upgrade dependencies or change the application runtime feature configuration.

## Alert 10: pointee lifetime and slot lifetime are distinct

The entire reported path is `drop_in_place::<T>(p as *mut T)` at line 898 followed
by `ptr::write(p, NULL)` at line 900. `PtrSlice` separately owns a GLib-allocated
buffer of pointer-sized slots. `reserve` obtains/resizes that buffer using
`g_realloc` (`ptr_slice.rs:702–725`). `truncate` first reduces `len`, then drops the
wrapper in a removed slot and writes NULL into that same still-allocated slot.
It neither frees the slot buffer nor changes its pointer/capacity.

For `Object`, `ObjectRef::drop` (`object.rs:389`) unrefs the separately allocated
`GObject` named by the stored pointer. Finalizing that object does not deallocate
the array slot holding its former pointer. `PtrSlice::drop` frees the slot buffer
only after dropping remaining live elements (`ptr_slice.rs:84–98`). This matches
Rust's distinction between [dropping a value in place](https://doc.rust-lang.org/std/ptr/fn.drop_in_place.html)
and deallocating its backing storage. Unsafe `TransparentPtrType` implementations
must still satisfy their representation and ownership contracts.

The concrete test truncates a slice containing externally shared and uniquely
owned objects. It observes correct refcounts and exactly-once finalization of the
unique tail, unchanged buffer address/capacity, NULL removed slots and terminator,
then successful buffer reuse. Truncation to zero and final slice/external-owner
destruction produce exactly four finalizations with no duplicate drops.

## Validation and initial review boundary

Run `cargo test --locked --release -p scope-glib-variant-regression` on Linux.
The follow-up adds 15 concrete cases (2 logging, 9 translation, 4 ownership),
alongside the existing nine optimized iterator regressions. These are
characterization tests of the reports' assumptions, not invented red/green
production fixes. All native calls use real installed GLib/GObject/GIO libraries.
No sanitizer or Miri result is claimed, nor a proof for every unsafe caller,
platform, FFI API or application logging path.

The initial triage required independent operator review of these proposed
dispositions. The source-authoring task changed no alert state, query, workflow
security configuration, vendor exclusion or allowlist.
The baseline here is exactly seven alerts; a new-head analysis is separate
evidence. The new concrete boxed derive exercises the same implementation as
`MyBoxed` and may introduce another test-only analysis location. Any newly reported
location requires its own exact-flow review, not automatic classification.
Platform checks and Windows verifier acceptance are reported separately in the PR.
Issue #11 remains open until interactive GTK/dialog acceptance has been tested.

## Independent operator decisions: 6 November 2026

Alerts 4–10 were independently reviewed against the complete baseline SARIF and
the concrete generic instantiations described above, then individually dismissed
as false positives with alert-specific explanations. The review did not change
CodeQL queries, vendor coverage or workflow security configuration.

Alert [11](https://github.com/dalkedev/scope/security/code-scanning/11), introduced
by the new test-only `CountedBoxed` derive, received a separate review against
[analysis 2114713790](https://api.github.com/repos/dalkedev/scope/code-scanning/analyses/2114713790)
at `eec083e227ffa4230d0fc186f4a3613b647aef41`. Its four charset/error paths again
conflate incompatible generic conversions: the derive accepts concrete
`CountedBoxed` pointers, while the reported callers produce strings or `Error`.
No connecting cast exists in those flows. The ownership tests exercise actual
boxed allocations, transfer modes and clone/drop counts. This separate alert was
individually dismissed as a false positive.

[The independent review record](https://github.com/dalkedev/scope/pull/27#issuecomment-6693586655)
links these decisions to their evidence. The eight decisions apply to those exact
reported flows; any newly introduced alert still requires its own review.
