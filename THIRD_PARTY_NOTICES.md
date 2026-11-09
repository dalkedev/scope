# Third-party notices

The project is distributed under **AGPL-3.0-or-later**; see [LICENSE](LICENSE).
Third-party components retain the copyright notices and license terms described below.
The project name does not imply affiliation with HTTP Toolkit or Frida.

## Rust, frontend packages and fonts

Collected license texts and package/source inventories are included in
[`resources/licenses/index.html`](resources/licenses/index.html),
[`rust.json`](resources/licenses/rust.json) and
[`frontend.json`](resources/licenses/frontend.json). These include the
Manrope and IBM Plex Mono font licenses and the Lucide icon license. The
Rust 1.99.1 standard-library notices are retained separately in
[`resources/licenses/rust-standard-library/`](resources/licenses/rust-standard-library/).
Each collection records its source lockfile hash; regeneration is described in
[`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md). Third-party terms remain in force.

## GLib compatibility backport

`vendor/glib` contains GLib **0.18.5**, copyright its gtk-rs contributors,
under its original [MIT license](vendor/glib/LICENSE) and
[copyright notice](vendor/glib/COPYRIGHT). Scope applies only gtk-rs upstream
commit `b5a4071e439bef2b5eea76c3aa25e5ae84839e34` to repair
`VariantStrIter` unsoundness (RUSTSEC-2024-0429). This is a Scope-maintained
backport; it is not an upstream 0.18.5 release update.

The [provenance and verification instructions](vendor/glib-provenance/README.md)
retain the pristine crates.io archive, exact upstream patch and input/source
hashes. Both directories must be retained in corresponding source archives.
GLib's original license and authorship remain applicable; linked native GNOME
libraries have their own license terms described in GLib's `COPYRIGHT`.

## Original Scope engine and historical migration

`resources/frida/scope-engine.js` and the Python worker are original Scope integration code under the project AGPL-3.0-or-later license. The current runtime no longer includes the six HTTP Toolkit hook/config files or its extracted system-CA installation template. The project previously distributed them from the owner's `SculptTechProject/bypass-scripts-adb` repository, originating in HTTP Toolkit Server v1.26.1 (`1abef0adc96d07bd64377e85e847025a55bac2ac`). Their historical copyright, provenance and AGPL notices remain in public Git history. No history rewrite or project license change is part of this migration. This is not a legally certified clean-room claim.

## Frida Java bridge

File: `resources/frida/frida-java-bridge.js`.

This is the prebuilt **frida-java-bridge 7.0.13** bundle distributed by HTTP Toolkit Server v1.26.1. Its content is unchanged. It is not relicensed as original HTTP Toolkit code.

- Upstream authors: Frida contributors.
- Declared upstream license: **LGPL-2.0 WITH WxWindows-exception-3.1**, as recorded in the [7.0.13 package manifest](https://github.com/frida/frida-java-bridge/blob/6ab2a63e4b02ab881f11ea06632cebea4b1b435e/package.json).
- Full license texts: [GNU Library GPL version 2](resources/frida/licenses/LGPL-2.0.txt) and [wxWindows Library Licence version 3.1, including its exception](resources/frida/licenses/WxWindows-Library-Licence-3.1.txt).
- Corresponding bridge source: [frida-java-bridge v7.0.13](https://github.com/frida/frida-java-bridge/tree/6ab2a63e4b02ab881f11ea06632cebea4b1b435e), commit `6ab2a63e4b02ab881f11ea06632cebea4b1b435e`; [source archive](https://github.com/frida/frida-java-bridge/archive/6ab2a63e4b02ab881f11ea06632cebea4b1b435e.tar.gz).
- Bundle source: [HTTP Toolkit Server v1.26.1, frida-java-bridge.js](https://github.com/httptoolkit/httptoolkit-server/blob/1abef0adc96d07bd64377e85e847025a55bac2ac/overrides/frida/frida-java-bridge.js).

The generation command provided in the bundle header uses `frida-compile` to bundle the following entry point:

```js
import Java from 'frida-java-bridge';
globalThis.Java ||= Java;
```

The bundle includes Buffer, Base64 and IEEE754 helper code from Frida's JavaScript shims. The original package notices are included here:

| Component | Authors / copyright | License text | Source |
| --- | --- | --- | --- |
| Buffer | Feross Aboukhadijeh and other contributors | [MIT](resources/frida/licenses/buffer-MIT.txt) | [frida/gumjs-buffer](https://github.com/frida/gumjs-buffer) |
| Base64 | Copyright (c) 2014 Jameson Little | [MIT](resources/frida/licenses/base64-MIT.txt) | [frida/gumjs-base64](https://github.com/frida/gumjs-base64) |
| IEEE754 | Copyright 2008 Fair Oaks Labs, Inc. | [BSD-3-Clause](resources/frida/licenses/ieee754-BSD-3-Clause.txt) | [frida/gumjs-ieee754](https://github.com/frida/gumjs-ieee754) |

The helper license texts were copied verbatim from the published `@frida/buffer@7.0.4`, `@frida/base64-js@2.0.3` and `@frida/ieee754@2.0.2` packages. These versions appear in the bridge's upstream 7.0.13 development lockfile; HTTP Toolkit does not include a build lockfile with its prebuilt bundle, so the precise helper versions embedded in that artifact are not asserted here.

The GNU Library GPL text was obtained from [GNU](https://www.gnu.org/licenses/old-licenses/lgpl-2.0.txt). The wxWindows license and exception were obtained from [Frida Gum 17.9.1 COPYING](https://github.com/frida/frida-gum/blob/17.9.1/COPYING), preserving that license section verbatim.

## Retained JavaScript integrity

The unchanged retained bridge distributed by HTTP Toolkit Server v1.26.1 has SHA-256 `88f0ca1fb34d2bd7b589f3d4c44aa0077780473530922887a53311ec55d9c214`. The exact bridge distribution source and corresponding source are listed above; retention does not make it original Scope code.

## Optional packaged Frida runtime

**Public alpha releases do not redistribute the native Frida worker, Python
runtime or Android Frida server.** `npm run package` stages filtered resources
that exclude those optional binaries. The user can explicitly run the shipped
`resources/device/install-runtime.py` helper to download Frida from PyPI and
the official Frida release into a private user runtime. That installation does
not modify system Python or an Android device.

A previously verified local macOS arm64 experiment included
`resources/bin/scope-frida-worker` and the unmodified official
`frida-server-17.9.1-android-arm64` binary. The following provenance is retained
for that local experiment and future review; those native components are absent
from public release artifacts and are not relicensed as Scope code.

| Component | Version / origin | License text |
| --- | --- | --- |
| Frida Python bindings | 17.9.1 | [wxWindows Library Licence 3.1](resources/device/licenses/frida-python-17.9.1-COPYING.txt) |
| Frida core / Android server | 17.9.1, official upstream binary | [wxWindows Library Licence 3.1](resources/device/licenses/frida-core-17.9.1-COPYING.txt) |
| Frida Gum | 17.9.1 | [wxWindows Library Licence 3.1 and embedded third-party notices](resources/device/licenses/frida-gum-17.9.1-COPYING.txt) |
| CPython | 3.12.13, python-build-standalone 20260610 | [Python license and historical notices](resources/device/licenses/python-3.12.13-LICENSE.txt), [runtime dependency licenses](resources/device/licenses/python-build-standalone-20260610/licenses) |
| PyInstaller bootloader / runtime | 6.22.3 | [GPL 2 or later with bootloader exception; Apache/MIT runtime terms](resources/device/licenses/pyinstaller-6.22.3-COPYING.txt) |

The main Frida licenses incorporate GNU Library GPL 2 or later, with an explicit
exception permitting binary object code distribution under the distributor's
own terms. The [GNU Library GPL text](resources/device/licenses/LGPL-2.0.txt)
is included alongside those exceptions. Scope does not modify Frida, CPython or
PyInstaller. The original Scope engine remains under the project AGPL license.

The older Python 3.9 developer environment optionally installs
`typing_extensions` 4.16.0; its [license](resources/device/licenses/typing_extensions-4.16.0-LICENSE.txt)
is retained for that case. It is absent from the Python 3.12 packaged worker.

Exact release sources, pinned Frida submodules, Python distribution metadata,
archive integrity and the public distribution boundary
are recorded in the [runtime notice index](resources/device/licenses/README.md).
Keep that directory with the integration sources. Any future redistribution of
the optional native runtime requires a complete artifact-specific dependency and
corresponding-source review. The public packaging checks currently reject it.

## Owned Android acceptance APK (test only)

The original [lab fixture](examples/android-engine-fixture/README.md) downloads checksum-locked OkHttp 3.14.9 and Okio 1.17.2 only into an explicit external build directory. Its APK includes [dependency notices](examples/android-engine-fixture/THIRD_PARTY_NOTICES.txt), Apache-2.0 text and any embedded JAR notices, including the Public Suffix List MPL-2.0 notice. These JARs and APKs are excluded from source control and Scope production resources.
