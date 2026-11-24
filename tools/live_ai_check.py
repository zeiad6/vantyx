#!/usr/bin/env python3
"""Opt-in live check for an OpenAI-compatible provider.

This harness never prints or stores the API key. It reads the key from the
environment variable named by ``--api-key-env`` and records only the variable
name in the evidence file.

It exercises the real ``vantyx ai`` boundary:

  * model discovery (``doctor``)
  * non-streamed and streamed chat
  * schema-validated extraction with bounded repair
  * embeddings with an explicit dimension
  * semantic RAG indexing, search, and retrieval evaluation

Usage:
    export GOOGLE_API_KEY=...
    python3 tools/live_ai_check.py \
        --endpoint https://generativelanguage.googleapis.com/v1beta/openai \
        --model gemini-3.1-flash-lite \
        --embedding-model gemini-embedding-001 \
        --dimensions 768 \
        --out docs/ai/live-evidence-2026-11-24.json

The harness makes billable provider requests. Run it only when that is
intended. It exits nonzero if a required check fails.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Any


def summarize_run(run: dict[str, Any], limit: int = 600) -> dict[str, Any]:
    """Bound evidence size while preserving exit status and error codes."""
    summary = {key: value for key, value in run.items() if key not in ("stdout", "stderr")}
    for stream in ("stdout", "stderr"):
        text = run.get(stream, "")
        if len(text) > limit:
            summary[stream] = text[:limit] + f"... [truncated {len(text) - limit} chars]"
        else:
            summary[stream] = text
    return summary


def is_retryable(run: dict[str, Any]) -> bool:
    stderr = run.get("stderr", "")
    return (
        "retryable" in stderr.lower()
        or "AI_E_RATE_LIMIT" in stderr
        or "AI_E_TIMEOUT" in stderr
        or ("AI_E_PROVIDER" in stderr and "HTTP 503" in stderr)
    )


def run_cli(
    binary: str,
    args: list[str],
    env: dict[str, str],
    timeout: float = 180.0,
    retries: int = 3,
    base_delay: float = 2.0,
) -> dict[str, Any]:
    history: list[dict[str, Any]] = []
    for attempt in range(1, retries + 1):
        started = time.monotonic()
        completed = subprocess.run(
            [binary, "ai", *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=timeout,
            check=False,
        )
        run = {
            "command": [binary, "ai", *args],
            "exit_code": completed.returncode,
            "duration_ms": round((time.monotonic() - started) * 1000),
            "stdout": completed.stdout,
            "stderr": completed.stderr,
        }
        if run["exit_code"] == 0 or attempt == retries or not is_retryable(run):
            run["retry_history"] = history
            return run
        history.append(
            {
                "attempt": attempt,
                "exit_code": run["exit_code"],
                "stderr": run["stderr"][:300],
            }
        )
        time.sleep(base_delay * (2 ** (attempt - 1)))
    raise AssertionError("retry loop exhausted without returning")


def run_streaming(
    binary: str,
    args: list[str],
    env: dict[str, str],
    timeout: float = 180.0,
) -> dict[str, Any]:
    started = time.monotonic()
    process = subprocess.Popen(
        [binary, "ai", *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    stderr_chunks: list[bytes] = []

    def drain_stderr() -> None:
        assert process.stderr is not None
        for chunk in iter(lambda: process.stderr.read(4096), b""):
            stderr_chunks.append(chunk)

    thread = threading.Thread(target=drain_stderr, daemon=True)
    thread.start()
    assert process.stdout is not None
    chunks: list[bytes] = []
    first_byte_ms: int | None = None
    deadline = started + timeout
    while True:
        if time.monotonic() > deadline:
            process.kill()
            raise TimeoutError("streaming check exceeded its timeout")
        byte = process.stdout.read(1)
        if not byte:
            break
        if first_byte_ms is None:
            first_byte_ms = round((time.monotonic() - started) * 1000)
        chunks.append(byte)
    exit_code = process.wait(timeout=30)
    thread.join(timeout=5)
    return {
        "command": [binary, "ai", *args],
        "exit_code": exit_code,
        "first_byte_ms": first_byte_ms,
        "duration_ms": round((time.monotonic() - started) * 1000),
        "stdout": b"".join(chunks).decode("utf-8", errors="replace"),
        "stderr": b"".join(stderr_chunks).decode("utf-8", errors="replace"),
    }


def check(record: dict[str, Any], name: str, ok: bool, detail: str = "") -> None:
    record["checks"].append({"name": name, "ok": ok, "detail": detail})
    if not ok:
        record["ok"] = False


def parse_json_output(run: dict[str, Any]) -> dict[str, Any]:
    try:
        value = json.loads(run["stdout"])
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def build_corpus(root: Path) -> Path:
    (root / "vantyx.md").write_text(
        "Vantyx is a gradual-typing language implemented in Rust. It provides a "
        "tree-walking interpreter as the semantic oracle and a native GNU C11 "
        "backend that invokes the system C compiler. Native behavior must match "
        "the interpreter, and unsupported constructs are rejected rather than "
        "miscompiled.\n",
        encoding="utf-8",
    )
    (root / "gemini.md").write_text(
        "Google Gemini exposes an OpenAI-compatible endpoint under "
        "generativelanguage.googleapis.com/v1beta/openai. The endpoint supports "
        "chat completions, model listing, and embedding vectors. Authentication "
        "uses a bearer API key, and model names are passed without the models/ "
        "prefix.\n",
        encoding="utf-8",
    )
    (root / "copy.md").write_text(
        "copy_dir_all copies an existing directory tree to a new target path. It "
        "follows a root source link, rejects inner symbolic links and special "
        "files, and does not preserve metadata. Trees above 100000 entries or 128 "
        "levels fail with R0015.\n",
        encoding="utf-8",
    )
    fixture = root.parent / "fixture.json"
    fixture.write_text(
        json.dumps(
            {
                "queries": [
                    {
                        "query": "Which engine compiles through the system C compiler?",
                        "relevant": ["vantyx.md"],
                    },
                    {
                        "query": "Where can I call chat completions and embeddings on Google's API?",
                        "relevant": ["gemini.md"],
                    },
                    {
                        "query": "Does recursive copy follow links inside the tree?",
                        "relevant": ["copy.md"],
                    },
                ]
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/vantyx")
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--model", default="gemini-3.1-flash-lite")
    parser.add_argument("--embedding-model", default="gemini-embedding-001")
    parser.add_argument("--api-key-env", default="GOOGLE_API_KEY")
    parser.add_argument("--dimensions", type=int, default=768)
    parser.add_argument("--schema", default="examples/ai/person.schema.json")
    parser.add_argument("--out", default="")
    parser.add_argument("--skip-rag", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    options = parser.parse_args()

    binary = os.path.abspath(options.binary)
    if not os.path.isfile(binary):
        print(f"error: binary not found: {binary}", file=sys.stderr)
        return 2
    key = os.environ.get(options.api_key_env, "")
    if not key.strip():
        print(
            f"error: environment variable {options.api_key_env} is unset or empty",
            file=sys.stderr,
        )
        return 2
    schema = os.path.abspath(options.schema)
    if not os.path.isfile(schema):
        print(f"error: schema not found: {schema}", file=sys.stderr)
        return 2

    env = dict(os.environ)
    common = [
        "--provider",
        "openai",
        "--endpoint",
        options.endpoint,
        "--model",
        options.model,
        "--api-key-env",
        options.api_key_env,
        "--allow-network",
    ]
    embedding_common = [
        "--provider",
        "openai",
        "--endpoint",
        options.endpoint,
        "--model",
        options.embedding_model,
        "--api-key-env",
        options.api_key_env,
        "--allow-network",
        "--dimensions",
        str(options.dimensions),
    ]
    record: dict[str, Any] = {
        "binary": binary,
        "endpoint": options.endpoint,
        "model": options.model,
        "embedding_model": options.embedding_model,
        "api_key_env": options.api_key_env,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "checks": [],
        "ok": True,
    }

    doctor = run_cli(binary, ["doctor", *common], env)
    record["doctor"] = summarize_run(doctor)
    check(record, "doctor exits 0", doctor["exit_code"] == 0, doctor["stderr"])
    check(record, "doctor reports ok", "status: ok" in doctor["stdout"], doctor["stdout"])
    check(record, "doctor lists models", "models:" in doctor["stdout"], doctor["stdout"])

    chat = run_cli(
        binary,
        [
            "chat",
            *common,
            "--temperature",
            "0",
            "--max-tokens",
            "64",
            "Reply with exactly VANTYX LIVE and no other text.",
        ],
        env,
    )
    record["chat"] = summarize_run(chat)
    chat_text = chat["stdout"].strip()
    record["chat_text"] = chat_text
    check(record, "chat exits 0", chat["exit_code"] == 0, chat["stderr"])
    check(
        record,
        "chat contains VANTYX LIVE",
        "VANTYX" in chat_text.upper() and "LIVE" in chat_text.upper(),
        chat_text,
    )

    chat_json = run_cli(
        binary,
        [
            "chat",
            *common,
            "--temperature",
            "0",
            "--max-tokens",
            "64",
            "--json",
            "Reply with exactly VANTYX LIVE and no other text.",
        ],
        env,
    )
    record["chat_json"] = summarize_run(chat_json)
    parsed_chat = parse_json_output(chat_json)
    record["chat_usage"] = parsed_chat.get("usage")
    check(record, "json chat exits 0", chat_json["exit_code"] == 0, chat_json["stderr"])
    check(
        record,
        "json chat reports usage",
        isinstance(parsed_chat.get("usage"), dict)
        and isinstance(parsed_chat["usage"].get("total_tokens"), int)
        and parsed_chat["usage"]["total_tokens"] > 0,
        chat_json["stdout"],
    )

    stream = run_streaming(
        binary,
        [
            "chat",
            *common,
            "--temperature",
            "0",
            "--max-tokens",
            "96",
            "--stream",
            "Count from one to five in words, separated by spaces, with no other text.",
        ],
        env,
    )
    record["stream"] = summarize_run(stream)
    stream_text = stream["stdout"].strip()
    record["stream_text"] = stream_text
    check(record, "stream exits 0", stream["exit_code"] == 0, stream["stderr"])
    check(record, "stream has a first byte", (stream["first_byte_ms"] or 0) > 0, str(stream))
    check(
        record,
        "stream contains counted words",
        "one" in stream_text.lower() and "five" in stream_text.lower(),
        stream_text,
    )

    extract = run_cli(
        binary,
        [
            "extract",
            *common,
            "--temperature",
            "0",
            "--schema",
            schema,
            "--json",
            "Extract the person record: Ada Lovelace is an engineer with 12 years "
            "of experience in mathematics and computing.",
        ],
        env,
    )
    record["extract"] = summarize_run(extract)
    parsed_extract = parse_json_output(extract)
    record["extract_value"] = parsed_extract.get("value")
    record["extract_attempts"] = parsed_extract.get("attempts")
    check(record, "extract exits 0", extract["exit_code"] == 0, extract["stderr"])
    value = parsed_extract.get("value")
    check(
        record,
        "extract returns a validated person",
        isinstance(value, dict)
        and isinstance(value.get("name"), str)
        and value.get("role") in ("engineer", "designer", "manager"),
        extract["stdout"],
    )
    check(
        record,
        "extract reports bounded attempts",
        isinstance(parsed_extract.get("attempts"), int)
        and 1 <= parsed_extract["attempts"] <= 3,
        extract["stdout"],
    )

    embed = run_cli(
        binary,
        [
            "embed",
            *embedding_common,
            "--json",
            "Vantyx is a gradual-typing language with a native backend.",
            "The quick brown fox jumps over the lazy dog.",
        ],
        env,
    )
    record["embed"] = summarize_run(embed)
    parsed_embed = parse_json_output(embed)
    vectors = parsed_embed.get("vectors")
    record["embed_dimensions"] = parsed_embed.get("dimensions")
    record["embed_count"] = len(vectors) if isinstance(vectors, list) else 0
    record["embed_usage"] = parsed_embed.get("usage")
    check(record, "embed exits 0", embed["exit_code"] == 0, embed["stderr"])
    check(
        record,
        "embed returns requested dimensions",
        parsed_embed.get("dimensions") == options.dimensions
        and isinstance(vectors, list)
        and len(vectors) == 2
        and all(isinstance(vector, list) and len(vector) == options.dimensions for vector in vectors),
        embed["stdout"][:200],
    )

    if not options.skip_rag:
        with tempfile.TemporaryDirectory(prefix="vantyx-live-rag-") as temporary:
            root = Path(temporary) / "corpus"
            root.mkdir()
            fixture = build_corpus(root)
            index_path = Path(temporary) / "index.json"
            index = run_cli(
                binary,
                [
                    "index",
                    "--root",
                    str(root),
                    "--out",
                    str(index_path),
                    *embedding_common,
                    "--chunk-chars",
                    "512",
                    "--json",
                ],
                env,
            )
            record["index"] = summarize_run(index)
            parsed_index = parse_json_output(index)
            record["index_stats"] = {
                key: parsed_index.get(key)
                for key in ("documents", "chunks", "vectors", "dimensions", "usage")
            }
            check(record, "index exits 0", index["exit_code"] == 0, index["stderr"])
            check(
                record,
                "index stores one vector per chunk",
                parsed_index.get("documents") == 3
                and parsed_index.get("chunks") == 3
                and parsed_index.get("vectors") == 3
                and parsed_index.get("dimensions") == options.dimensions,
                index["stdout"],
            )

            search = run_cli(
                binary,
                [
                    "search",
                    "--index",
                    str(index_path),
                    "--query",
                    "Which engine compiles through the system C compiler?",
                    *embedding_common,
                    "--mode",
                    "semantic",
                    "--top-k",
                    "3",
                    "--json",
                ],
                env,
            )
            record["search"] = summarize_run(search)
            parsed_search = parse_json_output(search)
            hits = parsed_search.get("hits", [])
            top_source = hits[0].get("source") if hits else None
            record["search_top_source"] = top_source
            check(record, "search exits 0", search["exit_code"] == 0, search["stderr"])
            check(
                record,
                "semantic search ranks the native-backend document first",
                top_source == "vantyx.md",
                str(hits[:1]),
            )

            evaluation = run_cli(
                binary,
                [
                    "eval",
                    "--index",
                    str(index_path),
                    "--fixture",
                    str(fixture),
                    *embedding_common,
                    "--mode",
                    "semantic",
                    "--top-k",
                    "2",
                    "--json",
                ],
                env,
            )
            record["eval"] = summarize_run(evaluation)
            parsed_eval = parse_json_output(evaluation)
            record["eval_metrics"] = {
                key: parsed_eval.get(key)
                for key in ("queries", "recall_at_k", "precision_at_k", "mrr")
            }
            check(record, "eval exits 0", evaluation["exit_code"] == 0, evaluation["stderr"])
            check(
                record,
                "eval recalls every relevant document",
                parsed_eval.get("queries") == 3 and parsed_eval.get("recall_at_k") == 1,
                evaluation["stdout"],
            )

    payload = json.dumps(record, indent=2) + "\n"
    if options.out:
        Path(options.out).parent.mkdir(parents=True, exist_ok=True)
        Path(options.out).write_text(payload, encoding="utf-8")
    if options.verbose or not record["ok"]:
        print(payload, end="")
    failures = [item for item in record["checks"] if not item["ok"]]
    if failures:
        print(f"FAIL: {len(failures)} check(s) failed", file=sys.stderr)
        return 1
    print(f"OK: {len(record['checks'])} live checks against {options.endpoint}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
