#!/usr/bin/env python3
"""Deterministic loopback harness for Vantyx AI failure paths.

The harness starts local OpenAI-compatible HTTP servers, runs the real
`vantyx ai` CLI against them, and records the observed exit codes and stable
error codes. It never contacts a remote host and never needs a real key.

Covered paths:
  * successful model listing and non-streamed chat
  * streamed chat deltas
  * HTTP 401 -> AI_E_AUTH
  * HTTP 429 -> AI_E_RATE_LIMIT (retryable)
  * an unresponsive endpoint -> AI_E_TIMEOUT

Usage:
    python3 tools/ai_failure_harness.py --binary target/debug/vantyx
    python3 tools/ai_failure_harness.py --out /tmp/ai-harness.json --verbose
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

SUCCESS_TEXT = "VANTYX OFFLINE OK"


class ScenarioHandler(BaseHTTPRequestHandler):
    """Small OpenAI-compatible server whose behavior is selected per instance."""

    scenario = "success"
    protocol_version = "HTTP/1.1"

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def _read_body(self) -> bytes:
        length = int(self.headers.get("Content-Length", "0") or "0")
        return self.rfile.read(length) if length else b""

    def _send_json(self, status: int, payload: dict[str, Any], extra: dict[str, str] | None = None) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        for name, value in (extra or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - stdlib handler name
        if self.path.endswith("/models"):
            if self.scenario == "auth":
                self._send_json(401, {"error": {"message": "invalid api key"}})
                return
            if self.scenario == "rate":
                self._send_json(429, {"error": {"message": "slow down"}}, {"Retry-After": "1"})
                return
            self._send_json(
                200,
                {
                    "object": "list",
                    "data": [{"id": "test-model", "object": "model"}],
                },
            )
            return
        self._send_json(404, {"error": {"message": "not found"}})

    def do_POST(self) -> None:  # noqa: N802 - stdlib handler name
        body = self._read_body()
        if self.scenario == "slow":
            time.sleep(5)
        if self.scenario == "auth":
            self._send_json(401, {"error": {"message": "invalid api key"}})
            return
        if self.scenario == "rate":
            self._send_json(429, {"error": {"message": "slow down"}}, {"Retry-After": "1"})
            return
        if not self.path.endswith("/chat/completions"):
            self._send_json(404, {"error": {"message": "not found"}})
            return
        request = json.loads(body or b"{}")
        if request.get("stream"):
            self._send_stream()
            return
        self._send_json(
            200,
            {
                "id": "chatcmpl-harness",
                "object": "chat.completion",
                "model": "test-model",
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": SUCCESS_TEXT},
                        "finish_reason": "stop",
                    }
                ],
                "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8},
            },
        )

    def _send_stream(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        for delta in ("VANTYX ", "OFFLINE ", "OK"):
            frame = {
                "id": "chatcmpl-harness",
                "object": "chat.completion.chunk",
                "model": "test-model",
                "choices": [{"index": 0, "delta": {"content": delta}, "finish_reason": None}],
            }
            self.wfile.write(f"data: {json.dumps(frame)}\n\n".encode("utf-8"))
            self.wfile.flush()
        final = {
            "id": "chatcmpl-harness",
            "object": "chat.completion.chunk",
            "model": "test-model",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8},
        }
        self.wfile.write(f"data: {json.dumps(final)}\n\n".encode("utf-8"))
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()


def start_server(scenario: str) -> tuple[ThreadingHTTPServer, str]:
    handler = type(f"Handler_{scenario}", (ScenarioHandler,), {"scenario": scenario})
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    return server, f"http://127.0.0.1:{port}/v1"


def run_cli(binary: str, args: list[str], env: dict[str, str], timeout: float = 20.0) -> dict[str, Any]:
    started = time.monotonic()
    completed = subprocess.run(
        [binary, "ai", *args],
        capture_output=True,
        text=True,
        env=env,
        timeout=timeout,
        check=False,
    )
    duration_ms = round((time.monotonic() - started) * 1000)
    return {
        "command": [binary, "ai", *args],
        "exit_code": completed.returncode,
        "duration_ms": duration_ms,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
    }


def check(record: dict[str, Any], name: str, ok: bool, detail: str = "") -> None:
    record["checks"].append({"name": name, "ok": ok, "detail": detail})
    if not ok:
        record["ok"] = False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/vantyx")
    parser.add_argument("--out", default="")
    parser.add_argument("--verbose", action="store_true")
    options = parser.parse_args()

    binary = os.path.abspath(options.binary)
    if not os.path.isfile(binary):
        print(f"error: binary not found: {binary}", file=sys.stderr)
        return 2

    env = dict(os.environ)
    env["VANTYX_TEST_KEY"] = "harness-placeholder-key"
    common = [
        "--provider",
        "openai",
        "--model",
        "test-model",
        "--api-key-env",
        "VANTYX_TEST_KEY",
    ]
    record: dict[str, Any] = {
        "binary": binary,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "scenarios": [],
        "checks": [],
        "ok": True,
    }

    servers: list[ThreadingHTTPServer] = []
    try:
        success_server, success_url = start_server("success")
        servers.append(success_server)
        auth_server, auth_url = start_server("auth")
        servers.append(auth_server)
        rate_server, rate_url = start_server("rate")
        servers.append(rate_server)
        slow_server, slow_url = start_server("slow")
        servers.append(slow_server)

        doctor = run_cli(binary, ["doctor", *common, "--endpoint", success_url], env)
        record["scenarios"].append({"name": "doctor_success", **doctor})
        check(record, "doctor exits 0", doctor["exit_code"] == 0, doctor["stderr"])
        check(record, "doctor lists one model", "models: 1" in doctor["stdout"], doctor["stdout"])

        chat = run_cli(
            binary,
            ["chat", *common, "--endpoint", success_url, "--temperature", "0", "Say the harness text."],
            env,
        )
        record["scenarios"].append({"name": "chat_success", **chat})
        check(record, "chat exits 0", chat["exit_code"] == 0, chat["stderr"])
        check(record, "chat returns expected text", SUCCESS_TEXT in chat["stdout"], chat["stdout"])

        chat_json = run_cli(
            binary,
            [
                "chat",
                *common,
                "--endpoint",
                success_url,
                "--json",
                "--temperature",
                "0",
                "Say the harness text.",
            ],
            env,
        )
        record["scenarios"].append({"name": "chat_json_success", **chat_json})
        check(record, "json chat exits 0", chat_json["exit_code"] == 0, chat_json["stderr"])
        parsed_json: dict[str, Any] = {}
        try:
            parsed_json = json.loads(chat_json["stdout"])
        except json.JSONDecodeError:
            pass
        check(record, "json chat text", parsed_json.get("text") == SUCCESS_TEXT, chat_json["stdout"])
        check(
            record,
            "json chat usage",
            parsed_json.get("usage", {}).get("total_tokens") == 8,
            chat_json["stdout"],
        )

        stream = run_cli(
            binary,
            [
                "chat",
                *common,
                "--endpoint",
                success_url,
                "--stream",
                "--temperature",
                "0",
                "Stream the harness text.",
            ],
            env,
        )
        record["scenarios"].append({"name": "chat_stream_success", **stream})
        check(record, "stream exits 0", stream["exit_code"] == 0, stream["stderr"])
        check(record, "stream returns expected text", SUCCESS_TEXT in stream["stdout"], stream["stdout"])

        auth = run_cli(binary, ["chat", *common, "--endpoint", auth_url, "hello"], env)
        record["scenarios"].append({"name": "chat_auth_failure", **auth})
        check(record, "401 exits nonzero", auth["exit_code"] != 0, auth["stdout"])
        check(record, "401 maps to AI_E_AUTH", "AI_E_AUTH" in auth["stderr"], auth["stderr"])

        rate = run_cli(binary, ["chat", *common, "--endpoint", rate_url, "hello"], env)
        record["scenarios"].append({"name": "chat_rate_limit", **rate})
        check(record, "429 exits nonzero", rate["exit_code"] != 0, rate["stdout"])
        check(
            record,
            "429 maps to AI_E_RATE_LIMIT",
            "AI_E_RATE_LIMIT" in rate["stderr"],
            rate["stderr"],
        )
        check(record, "429 is marked retryable", "retryable" in rate["stderr"], rate["stderr"])

        timed = run_cli(
            binary,
            ["chat", *common, "--endpoint", slow_url, "--timeout-ms", "300", "hello"],
            env,
        )
        record["scenarios"].append({"name": "chat_timeout", **timed})
        check(record, "timeout exits nonzero", timed["exit_code"] != 0, timed["stdout"])
        check(record, "timeout maps to AI_E_TIMEOUT", "AI_E_TIMEOUT" in timed["stderr"], timed["stderr"])
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()

    payload = json.dumps(record, indent=2) + "\n"
    if options.out:
        with open(options.out, "w", encoding="utf-8") as handle:
            handle.write(payload)
    if options.verbose or not record["ok"]:
        print(payload, end="")
    failures = [item for item in record["checks"] if not item["ok"]]
    if failures:
        print(f"FAIL: {len(failures)} check(s) failed", file=sys.stderr)
        return 1
    print(f"OK: {len(record['checks'])} checks across {len(record['scenarios'])} scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
