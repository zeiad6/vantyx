#!/usr/bin/env python3
"""End-to-end smoke test for streamed OpenAI-compatible chat.

The script starts loopback HTTP and HTTPS servers that emit server-sent
events with delays. It then runs `vantyx ai chat --stream` and checks that:

* the request asks for `stream: true` and `Accept: text/event-stream`;
* text deltas arrive before the server sends the final chunk;
* the aggregated stdout matches the non-streamed text.

HTTPS uses the system `curl` binary and a temporary self-signed certificate.
That path is skipped when `openssl` or `curl` is unavailable. No external
network, model, or API key is used.

Usage:
    python3 tools/streaming_smoke.py --binary target/debug/vantyx
"""

from __future__ import annotations

import argparse
import os
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time

CHUNKS = [
    b'data: {"choices":[{"delta":{"content":"stream-"},"finish_reason":null}]}\n\n',
    b'data: {"choices":[{"delta":{"content":"smoke"},"finish_reason":"stop"}]}\n\n',
    b'data: {"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}\n\n',
    b'data: [DONE]\n\n',
]
EXPECTED = "stream-smoke\n"


def read_request(conn: socket.socket) -> bytes:
    data = b""
    while b"\r\n\r\n" not in data:
        chunk = conn.recv(4096)
        if not chunk:
            raise RuntimeError("connection closed before request headers")
        data += chunk
    head, rest = data.split(b"\r\n\r\n", 1)
    length = 0
    for line in head.split(b"\r\n"):
        if line.lower().startswith(b"content-length:"):
            length = int(line.split(b":", 1)[1].strip())
    while len(rest) < length:
        chunk = conn.recv(4096)
        if not chunk:
            raise RuntimeError("connection closed before request body")
        rest += chunk
    return head + b"\r\n\r\n" + rest


def run_case(binary: str, tls: bool, cert: str | None, key: str | None) -> None:
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    port = listener.getsockname()[1]
    request_seen: dict[str, bytes] = {}

    context = None
    if tls:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)

    def serve() -> None:
        conn, _ = listener.accept()
        if context is not None:
            conn = context.wrap_socket(conn, server_side=True)
        request_seen["request"] = read_request(conn)
        conn.sendall(
            b"HTTP/1.1 200 OK\r\n"
            b"Content-Type: text/event-stream\r\n"
            b"Connection: close\r\n\r\n"
        )
        for chunk in CHUNKS:
            conn.sendall(chunk)
            time.sleep(0.3)
        if context is not None:
            try:
                conn = conn.unwrap()
            except OSError:
                pass
        conn.close()

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    scheme = "https" if tls else "http"
    command = [
        binary,
        "ai",
        "chat",
        "--provider",
        "openai",
        "--endpoint",
        f"{scheme}://127.0.0.1:{port}/v1",
        "--model",
        "stream-smoke",
        "--stream",
    ]
    if tls:
        command.append("--allow-network")
    command.append("hello")

    environment = os.environ.copy()
    if tls:
        environment["CURL_CA_BUNDLE"] = cert
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
    )
    started = time.monotonic()
    arrivals: list[tuple[float, bytes]] = []
    assert process.stdout is not None
    while True:
        byte = process.stdout.read(1)
        if not byte:
            break
        arrivals.append((time.monotonic() - started, byte))
    assert process.stderr is not None
    stderr = process.stderr.read().decode("utf-8", "replace")
    status = process.wait()
    text = b"".join(byte for _, byte in arrivals).decode("utf-8")
    if status != 0:
        raise RuntimeError(f"{scheme} chat failed with exit {status}: {stderr}")
    if text != EXPECTED:
        raise RuntimeError(f"{scheme} stdout was {text!r}, expected {EXPECTED!r}")
    if len(arrivals) < 2:
        raise RuntimeError(f"{scheme} produced no incremental output")
    gap = arrivals[-1][0] - arrivals[0][0]
    if gap < 0.2:
        raise RuntimeError(
            f"{scheme} output arrived in one batch (first {arrivals[0][0]:.2f}s, "
            f"last {arrivals[-1][0]:.2f}s)"
        )
    request = request_seen.get("request", b"")
    if b'"stream":true' not in request:
        raise RuntimeError(f"{scheme} request did not ask for streaming")
    if b'"include_usage":true' not in request:
        raise RuntimeError(f"{scheme} request did not ask for usage")
    if b"Accept: text/event-stream" not in request:
        raise RuntimeError(f"{scheme} request did not accept text/event-stream")
    print(
        f"OK: {scheme} streamed {len(arrivals)} bytes; "
        f"first delta at {arrivals[0][0]:.2f}s, last at {arrivals[-1][0]:.2f}s"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/vantyx")
    arguments = parser.parse_args()
    binary = os.path.abspath(arguments.binary)
    if not os.path.isfile(binary):
        print(f"error: binary not found: {binary}", file=sys.stderr)
        return 2

    run_case(binary, tls=False, cert=None, key=None)

    if shutil.which("openssl") is None or shutil.which("curl") is None:
        print("SKIP: HTTPS case needs openssl and curl")
        return 0
    with tempfile.TemporaryDirectory(prefix="vantyx-stream-") as directory:
        cert = os.path.join(directory, "cert.pem")
        key = os.path.join(directory, "key.pem")
        subprocess.run(
            [
                "openssl",
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key,
                "-out",
                cert,
                "-days",
                "1",
                "-nodes",
                "-subj",
                "/CN=127.0.0.1",
                "-addext",
                "subjectAltName=IP:127.0.0.1",
            ],
            check=True,
            capture_output=True,
        )
        run_case(binary, tls=True, cert=cert, key=key)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
