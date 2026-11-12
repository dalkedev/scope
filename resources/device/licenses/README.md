# Optional Android runtime notices

This folder records the provenance of Scope's prior local Frida packaging
experiment. Public alpha releases exclude the standalone worker, embedded Python
and Android `frida-server` binary. Users can explicitly install upstream Frida
into Snurpo's private user runtime with `device/install-runtime.py`; the public
application does not redistribute those native downloads. The integration source
and the third-party interception scripts keep the separate licenses described in the root
[`THIRD_PARTY_NOTICES.md`](../../../THIRD_PARTY_NOTICES.md).

## Exact license copies

| File | Primary source |
| --- | --- |
| `frida-python-17.9.1-COPYING.txt` | [frida-python 17.9.1 COPYING](https://github.com/frida/frida-python/blob/17.9.1/COPYING) |
| `frida-core-17.9.1-COPYING.txt` | [frida-core 17.9.1 COPYING](https://github.com/frida/frida-core/blob/17.9.1/COPYING) |
| `frida-gum-17.9.1-COPYING.txt` | [frida-gum 17.9.1 COPYING](https://github.com/frida/frida-gum/blob/17.9.1/COPYING), including its embedded third-party notices |
| `LGPL-2.0.txt` | [GNU Library General Public License version 2](https://www.gnu.org/licenses/old-licenses/lgpl-2.0.txt) |
| `pyinstaller-6.22.3-COPYING.txt` | [PyInstaller v6.22.3 COPYING](https://github.com/pyinstaller/pyinstaller/blob/v6.22.3/COPYING.txt), including its bootloader exception, GPL, Apache and MIT texts |
| `python-3.12.13-LICENSE.txt` | [CPython v3.12.13 LICENSE](https://github.com/python/cpython/blob/v3.12.13/LICENSE), also copied from the actual packaged runtime |
| `typing_extensions-4.16.0-LICENSE.txt` | [typing_extensions 4.16.0 LICENSE](https://github.com/python/typing_extensions/blob/4.16.0/LICENSE); optional dependency for the older Python 3.9 developer environment, absent from the Python 3.12 packaged worker |

Frida Python, PyInstaller and typing_extensions license copies were compared
byte-for-byte with their installed wheel license files. The Python license was
copied from the actual task-local runtime; it matches the upstream version.

## Python runtime and its dependencies

The macOS arm64 worker uses CPython **3.12.13**, built by
[`astral-sh/python-build-standalone` release 20260610](https://github.com/astral-sh/python-build-standalone/releases/tag/20260610).
It does not embed the Python framework supplied with Xcode. Scope does not
modify the interpreter; PyInstaller packages it with the worker. The standalone
distribution has its own upstream build patches and configuration, available in
the release's source repository.

`python-build-standalone-20260610/PYTHON.json` and the complete `licenses/`
collection are preserved from the corresponding arm64 macOS `pgo+lto` full
archive, since an `install_only` runtime omits this metadata. They describe the
interpreter distribution and its native dependencies; not every optional Python
module or license in that collection is necessarily used by the worker.

- Full archive: [cpython-3.12.13+20260610-aarch64-apple-darwin-pgo+lto-full.tar.zst](https://github.com/astral-sh/python-build-standalone/releases/download/20260610/cpython-3.12.13%2B20260610-aarch64-apple-darwin-pgo%2Blto-full.tar.zst)
- Verified archive SHA-256: `5fb9c060fad883efe21e35715f2d30598ed644ab8f3d5045fb8ef2a0867b5be9`.
- Build source: [20260610 source archive](https://github.com/astral-sh/python-build-standalone/archive/refs/tags/20260610.tar.gz).
- CPython source: [v3.12.13 source archive](https://github.com/python/cpython/archive/refs/tags/v3.12.13.tar.gz).

## Frida source and binary provenance

The unmodified Android arm64 server comes from the official
[Frida 17.9.1 release](https://github.com/frida/frida/releases/tag/17.9.1).
The compressed upstream artifact is
[`frida-server-17.9.1-android-arm64.xz`](https://github.com/frida/frida/releases/download/17.9.1/frida-server-17.9.1-android-arm64.xz),
whose GitHub release SHA-256 is
`9200b4f31a6cf2aa8e847201e70ffa2c9621f5990d7a3fd96d5ed627c6f2d396`.
Scope only decompresses that binary before packaging it.
The packaged decompressed server was compared with that release artifact and
has SHA-256 `7dc6b75d0fa8cd0b90ffb8594dc7546930edee57a8839b92749ec58d609f7c4d`.

Frida's release source is pinned to commit
[`31d0d9a6f1b0b8abcccfc2ca10907381e97851f8`](https://github.com/frida/frida/tree/31d0d9a6f1b0b8abcccfc2ca10907381e97851f8).
The release's pinned submodules include:

- [frida-core `a62376a3d6319fcdbc8c6b81b6879634be71929b`](https://github.com/frida/frida-core/tree/a62376a3d6319fcdbc8c6b81b6879634be71929b).
- [frida-gum `8bf4e039daafb689849267402a50a42f24e33868`](https://github.com/frida/frida-gum/tree/8bf4e039daafb689849267402a50a42f24e33868).
- [frida-python `86d4b4b4bfa216b88e490de65093a1ffe938b546`](https://github.com/frida/frida-python/tree/86d4b4b4bfa216b88e490de65093a1ffe938b546).
- [build infrastructure `5595d3170560b98590dc0cb98d0d1c3f67e6e32d`](https://github.com/frida/releng/tree/5595d3170560b98590dc0cb98d0d1c3f67e6e32d).

To obtain the full source checkout, including the pinned submodules, use
`git clone --recurse-submodules --branch 17.9.1 https://github.com/frida/frida.git`.
A GitHub top-level source archive alone does not contain submodule contents.
The main Frida components use the wxWindows Library Licence 3.1, based on the
GNU Library GPL 2 or later with its binary-distribution exception. The license
copies above retain that exception verbatim. Snurpo does not modify Frida.

## Distribution scope

Snurpo's release boundary is enforced by `scripts/public-resources.mjs`: no
`scope-frida-worker` or `frida-server-*` binary is copied into public packages,
even when local ignored experimental binaries exist. The public setup helper
downloads the official version only when the user runs it, validates its pinned
archive SHA-256 and creates a virtualenv without changing system packages.

This choice follows the exact [Frida 17.9.1 SDK dependency manifest](https://github.com/frida/releng/blob/5595d3170560b98590dc0cb98d0d1c3f67e6e32d/deps.toml),
version `20260311`. The native SDK contains more than 30 projects, including
GLib, libiconv, libnice and V8; top-level Frida license copies alone do not cover
all linked dependency notices or corresponding-source/relink requirements.
Public releases therefore ship the integration and setup source, with an
explicit user-side installation, rather than assert that this historical local
bundle has a complete native distribution inventory.

`scripts/bundle-frida.py --local-only` remains available for local experiments.
Redistributing its output would require an artifact-specific dependency and
corresponding-source review first. That output is not a supported public release
asset in this alpha. See `docs/RELEASING.md` for the enforced release process.
