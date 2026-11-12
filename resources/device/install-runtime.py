#!/usr/bin/env python3
"""Install optional Frida into Snurpo's private user runtime, without system pip.

This is an explicit user-side download from PyPI and Frida's official release.
It does not contact, install to, or start an Android device. Snurpo pushes/starts
the server only when the user starts interception for a selected device/app.
"""
import argparse
import hashlib
import json
import lzma
import os
from pathlib import Path
import subprocess
import sys
import urllib.request
import venv

VERSION = "17.9.1"
ARCHES = {"arm64-v8a": "arm64", "armeabi-v7a": "arm", "x86": "x86", "x86_64": "x86_64"}
# SHA-256 of the compressed assets published by the official Frida 17.9.1 release.
HASHES = {
    "arm64": "9200b4f31a6cf2aa8e847201e70ffa2c9621f5990d7a3fd96d5ed627c6f2d396",
    "arm": "5f9e16f93d8dd7b1745e1e328835113a92d9e0795030d1c3069a234a6077c696",
    "x86": "c5bab5aa2272d74e0d1fc3d24772e27824284ec0b41ebb87539a36261343a65a",
    "x86_64": "f5f1205ffb3cb145de9264a8c6aa7aa8724a2e33df319d476f2ef67cac453c5f",
}


def default_runtime():
    if os.environ.get("SCOPE_DATA_DIR"):
        return Path(os.environ["SCOPE_DATA_DIR"]).expanduser().resolve() / "device"
    home = Path.home()
    if sys.platform == "darwin":
        data = home / "Library/Application Support/dev.scope.Scope"
    elif sys.platform == "win32":
        data = Path(os.environ.get("APPDATA", home / "AppData/Roaming")) / "scope/Scope/data"
    else:
        data = Path(os.environ.get("XDG_DATA_HOME", home / ".local/share")) / "scope"
    return data / "device"


def python_path(environment):
    return environment / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")


def download_server(arch, directory):
    name = "frida-server-%s-android-%s" % (VERSION, arch)
    url = "https://github.com/frida/frida/releases/download/%s/%s.xz" % (VERSION, name)
    cached = directory / (name + ".xz")
    payload = cached.read_bytes() if cached.is_file() else b""
    if hashlib.sha256(payload).hexdigest() != HASHES[arch]:
        print("Downloading official Android %s server..." % arch, flush=True)
        request = urllib.request.Request(url, headers={"User-Agent": "Snurpo-runtime-setup/0.1"})
        with urllib.request.urlopen(request, timeout=60) as response:
            payload = response.read(100 * 1024 * 1024)
        if hashlib.sha256(payload).hexdigest() != HASHES[arch]:
            raise RuntimeError("Frida server integrity check failed; no executable was installed")
        cached.write_bytes(payload)
    binary = lzma.decompress(payload)
    destination = directory / name
    temporary = directory / (name + ".tmp")
    temporary.write_bytes(binary)
    temporary.chmod(0o755)
    temporary.replace(destination)
    return {"path": name, "source": url, "sha256": hashlib.sha256(binary).hexdigest(), "archiveSha256": HASHES[arch]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--abi", choices=sorted(ARCHES), required=True, help="From adb shell getprop ro.product.cpu.abi")
    parser.add_argument("--runtime-dir", type=Path, default=default_runtime(), help="Snurpo's device runtime directory")
    args = parser.parse_args()
    if sys.version_info < (3, 9):
        parser.error("Python 3.9 or newer is required for this optional setup")
    root = args.runtime_dir.expanduser().resolve() / "managed-frida"
    root.mkdir(parents=True, exist_ok=True)
    environment = root / "venv"
    executable = python_path(environment)
    if not executable.is_file():
        print("Creating private virtualenv at %s" % environment, flush=True)
        venv.EnvBuilder(with_pip=True).create(environment)
    process_environment = os.environ.copy()
    process_environment.update({"PIP_DISABLE_PIP_VERSION_CHECK": "1", "PIP_CACHE_DIR": str(root / "pip-cache")})
    subprocess.run([
        str(executable), "-m", "pip", "install", "--only-binary=:all:", "--index-url", "https://pypi.org/simple",
        "frida==" + VERSION, "typing_extensions==4.16.0; python_version < '3.11'",
    ], check=True, env=process_environment)
    version = subprocess.check_output([str(executable), "-c", "import frida; print(frida.__version__)"], text=True).strip()
    if version != VERSION:
        raise RuntimeError("Installed Frida version did not match the requested version")
    server = download_server(ARCHES[args.abi], root)
    info = {
        "version": version, "abi": args.abi, "server": server,
        "pythonPackageSource": "https://pypi.org/project/frida/" + VERSION + "/",
        "licenseSource": "https://github.com/frida/frida/tree/" + VERSION,
        "setup": "Explicit user-side installation; no Android device changed",
    }
    (root / "runtime-info.json").write_text(json.dumps(info, indent=2) + "\n")
    print("Ready. Reopen Snurpo or press Start interception again. No environment variables are needed.", flush=True)
    print("Runtime: %s" % root, flush=True)


if __name__ == "__main__":
    main()
