#!/usr/bin/env python3
"""Download a matching Frida Android server locally; never install or start it.

Run with the same Python configured as SCOPE_PYTHON. Pass --abi arm64-v8a,
armeabi-v7a, x86 or x86_64 from `adb shell getprop ro.product.cpu.abi`.
The downloaded binary is passed to Snurpo as SCOPE_FRIDA_SERVER_LOCAL; only
Start interception copies and launches it on the explicitly selected device.
"""
import argparse
import lzma
from pathlib import Path
import urllib.request

def main():
    import frida
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--abi", required=True, choices=["arm64-v8a", "armeabi-v7a", "x86", "x86_64"])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    arch = {"arm64-v8a": "arm64", "armeabi-v7a": "arm", "x86": "x86", "x86_64": "x86_64"}[args.abi]
    version = frida.__version__
    if not all(part.isdigit() for part in version.split(".")):
        raise ValueError("Use an official numeric Frida release")
    url = "https://github.com/frida/frida/releases/download/%s/frida-server-%s-android-%s.xz" % (version, version, arch)
    with urllib.request.urlopen(url, timeout=60) as response:
        payload = response.read(100 * 1024 * 1024)
    binary = lzma.decompress(payload)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(binary)
    args.output.chmod(0o755)
    print("Downloaded Frida %s Android %s server to %s" % (version, arch, args.output.resolve()))
    print("Set SCOPE_FRIDA_SERVER_LOCAL to this path before launching Snurpo.")

if __name__ == "__main__":
    main()
