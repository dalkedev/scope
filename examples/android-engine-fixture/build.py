#!/usr/bin/env python3
"""Build only the owned lab APK; requires an existing SDK and JDK, no installers."""
import argparse
import hashlib
import json
import os
import re
import ssl
from pathlib import Path
import shutil
import subprocess
import time
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
DEPENDENCIES = (
    ('okhttp-3.14.9.jar', 'https://repo.maven.apache.org/maven2/com/squareup/okhttp3/okhttp/3.14.9/okhttp-3.14.9.jar', '2570fab55515cbf881d7a4ceef49fc515490bc027057e666776a2832465aeca0'),
    ('okio-1.17.2.jar', 'https://repo.maven.apache.org/maven2/com/squareup/okio/okio/1.17.2/okio-1.17.2.jar', 'f80ce42d2ffac47ad4c47e1d6f980d604d247ceb1a886705cf4581ab0c9fe2b8'),
)


def validate_output(path):
    path = Path(path).resolve()
    if path == REPO or REPO in path.parents or path in REPO.parents:
        raise ValueError('Generated output must be outside the source repository')
    return path


def prepare_output(path):
    output = validate_output(path)
    marker = output / '.scope-engine-fixture-build'
    identity = 'scope-engine-fixture-v1\n'
    if output.exists():
        if not output.is_dir():
            raise ValueError('Output must be a dedicated build directory')
        owned = marker.is_file() and not marker.is_symlink() and marker.stat().st_size == len(identity) and marker.read_text() == identity
        if any(output.iterdir()) and not owned:
            raise ValueError('Refusing nonempty unowned output directory')
    else:
        output.mkdir(parents=True, mode=0o700)
    # Ownership is established before changing permissions or removing children.
    os.chmod(output, 0o700)
    if not marker.exists():
        marker.write_text(identity)
    return output


def verify_jar(path, expected):
    path = Path(path)
    if path.stat().st_size > 2 * 1024 * 1024 or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise ValueError('SHA-256 mismatch: ' + path.name)


def read_public_ca(path):
    path = Path(path)
    if path.stat().st_size > 65536:
        raise ValueError('Expected one public PEM certificate, at most 64 KiB')
    pem = path.read_text(encoding='ascii')
    if not re.fullmatch(r'-----BEGIN CERTIFICATE-----\s+[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----\s*', pem, flags=re.ASCII):
        raise ValueError('Expected one public PEM certificate without private keys')
    try:
        der = ssl.PEM_cert_to_DER_cert(pem)
        # PEM decoding alone accepts arbitrary ASN.1, including relabelled keys.
        # An empty context parses actual X.509 CAs without trusting system roots.
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(cadata=pem)
        anchors = context.get_ca_certs(binary_form=True)
        if len(anchors) != 1 or anchors[0] != der:
            raise ValueError('Expected exactly one complete public CA certificate')
    except (ValueError, ssl.SSLError) as error:
        raise ValueError('Expected one valid complete public PEM CA certificate') from None
    return pem


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Dependency redirects are not permitted')


def acquire(output, offline):
    jars = []
    for name, url, digest in DEPENDENCIES:
        target = output / name
        if not target.exists():
            if offline:
                raise ValueError('Missing locked dependency in offline mode: ' + name)
            temporary = target.with_suffix('.part')
            deadline = time.monotonic() + 45
            try:
                opener = urllib.request.build_opener(NoRedirect)
                with opener.open(url, timeout=15) as response, temporary.open('xb') as stream:
                    total = 0
                    while True:
                        data = response.read1(65536)
                        if not data:
                            break
                        total += len(data)
                        if total > 2 * 1024 * 1024 or time.monotonic() > deadline:
                            raise ValueError('Dependency download exceeded bounds')
                        stream.write(data)
                verify_jar(temporary, digest)
                temporary.replace(target)
            finally:
                temporary.unlink(missing_ok=True)
        verify_jar(target, digest)  # Cached artifacts are checked on every build.
        jars.append(target)
    return jars


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--jdk', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--scope-ca', type=Path, help='Optional public fixture CA for the explicit okhttp-scope pin control')
    args = parser.parse_args()
    public_ca = read_public_ca(args.scope_ca) if args.scope_ca else None
    output = prepare_output(args.output)
    jars = acquire(output, args.offline)  # No build tool runs until both hashes pass.
    sdk, jdk = args.sdk.resolve(), args.jdk.resolve()
    tools = sdk / 'build-tools/36.0.0'
    android = sdk / 'platforms/android-35/android.jar'
    for path in [android, tools / 'aapt', tools / 'd8', tools / 'zipalign', tools / 'apksigner', jdk / 'bin/javac', jdk / 'bin/keytool']:
        if not path.is_file():
            raise ValueError('Required supplied tool missing: ' + str(path))
    env = os.environ.copy()
    env['JAVA_HOME'] = str(jdk)
    env['PATH'] = str(jdk / 'bin') + os.pathsep + env.get('PATH', '')

    def run(*command):
        result = subprocess.run([str(x) for x in command], env=env, cwd=output, capture_output=True, text=True, timeout=90, check=True)
        return (result.stdout + result.stderr).strip()

    classes, dex, assets = [output / name for name in ['classes', 'dex', 'assets']]
    for directory in [classes, dex, assets]:
        shutil.rmtree(directory, ignore_errors=True)
        directory.mkdir()
    if public_ca:
        (assets / 'scope-ca.pem').write_text(public_ca)
    notices = assets / 'third-party'
    notices.mkdir()
    shutil.copyfile(ROOT / 'THIRD_PARTY_NOTICES.txt', notices / 'NOTICE.txt')
    shutil.copyfile(REPO / 'LICENSE', assets / 'Scope-AGPL-3.0.txt')
    shutil.copyfile(REPO / 'resources/licenses/rust-standard-library/licenses/Apache-2.0.txt', notices / 'Apache-2.0.txt')
    for jar in jars:
        with zipfile.ZipFile(jar) as archive:
            for index, entry in enumerate(archive.infolist()):
                if any(word in entry.filename.lower() for word in ['license', 'notice']):
                    # Keep any embedded notices without trusting archive paths.
                    (notices / (jar.stem + '-' + str(index) + '.txt')).write_bytes(archive.read(entry))
    classpath = os.pathsep.join(str(path) for path in [android, *jars])
    run(jdk / 'bin/javac', '-source', '8', '-target', '8', '-Xlint:-options', '-cp', classpath, '-d', classes, *sorted((ROOT / 'src').rglob('*.java')))
    classjar = output / 'fixture-classes.jar'
    with zipfile.ZipFile(classjar, 'w') as archive:
        for path in classes.rglob('*.class'):
            archive.write(path, path.relative_to(classes))
    run(tools / 'd8', '--min-api', '26', '--lib', android, '--output', dex, classjar, *jars)
    unsigned = output / 'unsigned.apk'
    run(tools / 'aapt', 'package', '-f', '-M', ROOT / 'AndroidManifest.xml', '-I', android, '-A', assets, '-F', unsigned)
    with zipfile.ZipFile(unsigned, 'a', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(dex / 'classes.dex', 'classes.dex')
    aligned, signed, keystore = [output / name for name in ['aligned.apk', 'scope-engine-fixture.apk', 'fixture.p12']]
    run(tools / 'zipalign', '-f', '4', unsigned, aligned)
    if not keystore.exists():
        # Public disposable lab password; this key has no production identity.
        run(jdk / 'bin/keytool', '-genkeypair', '-keystore', keystore, '-storetype', 'PKCS12', '-storepass', 'android', '-keypass', 'android', '-alias', 'fixture', '-keyalg', 'RSA', '-keysize', '2048', '-validity', '7', '-dname', 'CN=Scope disposable fixture')
        os.chmod(keystore, 0o600)
    run(tools / 'apksigner', 'sign', '--ks', keystore, '--ks-key-alias', 'fixture', '--ks-pass', 'pass:android', '--key-pass', 'pass:android', '--out', signed, aligned)
    verification = run(tools / 'apksigner', 'verify', '--verbose', '--print-certs', signed)
    receipt = {
        'artifact': signed.name, 'sha256': hashlib.sha256(signed.read_bytes()).hexdigest(),
        'dependencies': {name: digest for name, _url, digest in DEPENDENCIES},
        'javac': run(jdk / 'bin/javac', '-version'), 'build_tools': '36.0.0', 'platform': 'android-35',
        'signature_verification': verification, 'android_execution': 'NOT RUN',
        'fixture_ca_sha256': hashlib.sha256(public_ca.encode('ascii')).hexdigest() if public_ca else None,
    }
    (output / 'build-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, indent=2))


if __name__ == '__main__':
    main()
