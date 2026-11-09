#!/usr/bin/env python3
"""Finite synthetic loopback HTTP/TLS lab. Keys live only in a temporary directory."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
import http.server
import json
import os
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading
import time

CASES = ('scope', 'foreign', 'wrong-host', 'expired')


def generate_fixtures(root):
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(root, 0o700)

    def openssl(*args):
        subprocess.run(['openssl', *map(str, args)], cwd=root, check=True, timeout=20, capture_output=True)

    for ca in ['scope', 'foreign']:
        openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '2', '-subj', '/CN=Scope synthetic ' + ca + ' CA', '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign', '-keyout', ca + '-ca.key', '-out', ca + '-ca.pem')
    for case in CASES:
        ca = 'foreign' if case == 'foreign' else 'scope'
        host = 'IP:127.0.0.2' if case == 'wrong-host' else 'IP:127.0.0.1'
        ext = root / (case + '.ext')
        ext.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=' + host + '\n')
        openssl('req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=Scope synthetic ' + case, '-keyout', case + '.key', '-out', case + '.csr')
        if case != 'expired':
            openssl('x509', '-req', '-in', case + '.csr', '-CA', ca + '-ca.pem', '-CAkey', ca + '-ca.key', '-CAcreateserial', '-days', '1', '-extfile', ext, '-out', case + '.pem')
        else:
            (root / 'index').touch()
            (root / 'serial').write_text('1000\n')
            (root / 'ca.cnf').write_text('[ca]\ndefault_ca=lab\n[lab]\ndatabase=index\nserial=serial\nnew_certs_dir=.\ncertificate=scope-ca.pem\nprivate_key=scope-ca.key\ndefault_md=sha256\npolicy=policy\n[policy]\ncommonName=supplied\n')
            now = datetime.now(timezone.utc)
            openssl('ca', '-batch', '-config', 'ca.cnf', '-in', case + '.csr', '-out', case + '.pem', '-extfile', ext,
                    '-startdate', (now - timedelta(days=3)).strftime('%Y%m%d%H%M%SZ'), '-enddate', (now - timedelta(days=1)).strftime('%Y%m%d%H%M%SZ'))
    for path in root.glob('*.key'):
        os.chmod(path, 0o600)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'scope-engine-fixture\n'
        self.send_response(200)
        self.send_header('Content-Type', 'text/plain')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *args):
        pass  # Never record request URLs, headers, bodies or real account data.


class LabServer(http.server.HTTPServer):
    def get_request(self):
        connection, address = super().get_request()
        self.connection_count += 1
        connection.settimeout(2)
        try:
            if self.tls_context:
                connection = self.tls_context.wrap_socket(connection, server_side=True)
            return connection, address
        except BaseException:
            connection.close()
            raise

    def handle_error(self, request, client_address):
        pass  # Rejected handshakes are expected negative controls.


@contextmanager
def running_servers(root, base_port, http_port, seconds, max_connections):
    if not (0 < seconds <= 600 and 1 <= max_connections <= 200):
        raise ValueError('Use 0 < seconds <= 600 and 1 <= max-connections <= 200')
    if not (0 <= base_port <= 65532 and 0 <= http_port <= 65535):
        raise ValueError('Invalid fixture ports')
    servers, threads, urls = [], [], {}
    stop = threading.Event()
    deadline = time.monotonic() + seconds

    def run(server):
        try:
            while not stop.is_set() and time.monotonic() < deadline and server.connection_count < max_connections:
                server.handle_request()
        finally:
            server.server_close()

    try:
        for index, case in enumerate((*CASES, 'http')):
            port = http_port if case == 'http' else (base_port + index if base_port else 0)
            server = LabServer(('127.0.0.1', port), Handler)
            servers.append(server)
            server.timeout = 0.2
            server.connection_count = 0
            server.tls_context = None
            if case != 'http':
                context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                context.load_cert_chain(Path(root) / (case + '.pem'), Path(root) / (case + '.key'))
                server.tls_context = context
            urls[case] = ('http' if case == 'http' else 'https') + '://127.0.0.1:' + str(server.server_port) + '/'
            thread = threading.Thread(target=run, args=(server,), daemon=True)
            thread.start()
            threads.append(thread)
        yield urls
    finally:
        stop.set()
        for thread in threads:
            thread.join(timeout=5)
        for server in servers:
            server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-port', type=int, default=18443)
    parser.add_argument('--http-port', type=int, default=18080)
    parser.add_argument('--seconds', type=float, default=120)
    parser.add_argument('--max-connections', type=int, default=100)
    parser.add_argument('--parent', type=Path, help='Existing private temporary parent directory')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='scope-engine-tls-', dir=args.parent) as temporary:
        root = Path(temporary)
        generate_fixtures(root)
        with running_servers(root, args.base_port, args.http_port, args.seconds, args.max_connections) as urls:
            print(json.dumps({'fixture': 'scope-engine-v1', 'urls': urls, 'scope_ca': str(root / 'scope-ca.pem'), 'seconds': args.seconds, 'android_execution': 'NOT RUN'}), flush=True)
            try:
                deadline = time.monotonic() + args.seconds
                while time.monotonic() < deadline:
                    time.sleep(min(0.2, deadline - time.monotonic()))
            except KeyboardInterrupt:
                pass


if __name__ == '__main__':
    main()
