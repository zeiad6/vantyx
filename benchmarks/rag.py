#!/usr/bin/env python3
"""Measure the local RAG index/search path with the offline hash provider.

This is a plumbing benchmark, not a model-quality benchmark. It measures:
  * index build time (ingestion, chunking, hash embeddings, JSON save),
  * one-shot search time (index load, query embedding, hybrid ranking),
  * no-op and one-changed-chunk incremental updates (vector reuse),
  * index size, document/chunk counts, and top-k hits.

The corpus is generated deterministically, so the benchmark is reproducible
without downloading data. Every run uses the same generated text and query.
One unmeasured warm-up pair is followed by N measured pairs; medians are
reported. The hash provider is lexical, not a trained semantic model, and flat
search is linear. Do not compare these numbers to a vector database or a live
model.

Usage:
    cargo build --release
    python3 benchmarks/rag.py --reps 5
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RESULTS = ROOT / "benchmarks" / "results"
QUERY = "compiler memory safety native map retrieval"


def build_corpus(root: Path, docs: int) -> None:
    root.mkdir(parents=True, exist_ok=True)
    topics = [
        "compiler pipeline diagnostics and type checking",
        "native backend allocation and memory safety",
        "retrieval augmented generation with cited chunks",
        "embedding provider limits and deterministic caches",
        "agent tool capabilities and least privilege",
        "language modules imports and declaration flattening",
    ]
    for index in range(docs):
        topic = topics[index % len(topics)]
        lines = [
            f"# Document {index:04d}",
            "",
            f"Topic: {topic}.",
            "",
        ]
        for line in range(18):
            lines.append(
                f"Line {line:02d} discusses {topic}, example token "
                f"rag{index:04d}x{line:02d}, and reproducible local behavior."
            )
        (root / f"doc-{index:04d}.md").write_text("\n".join(lines) + "\n")


def run_json(command: list[str], timeout: float) -> tuple[dict, float]:
    start = time.perf_counter()
    result = subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    elapsed = time.perf_counter() - start
    if result.returncode != 0:
        raise SystemExit(
            "error: command failed\n"
            f"command: {' '.join(command)}\n"
            f"stdout: {result.stdout}\n"
            f"stderr: {result.stderr}"
        )
    try:
        return json.loads(result.stdout), elapsed
    except json.JSONDecodeError as error:
        raise SystemExit(
            f"error: expected JSON from {' '.join(command)}: {error}\n{result.stdout}"
        ) from error


def stats(values: list[float]) -> dict[str, float]:
    return {
        "median_ms": round(statistics.median(values) * 1000.0, 3),
        "min_ms": round(min(values) * 1000.0, 3),
        "max_ms": round(max(values) * 1000.0, 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=str(ROOT / "target" / "release" / "vantyx"))
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument("--docs", type=int, default=200)
    parser.add_argument("--dimensions", type=int, default=64)
    parser.add_argument("--top-k", type=int, default=10)
    parser.add_argument("--out", default=None, help="JSON output path")
    args = parser.parse_args()

    binary = Path(args.binary).resolve()
    if not binary.is_file():
        raise SystemExit(f"error: binary not found: {binary}")
    if args.reps < 1 or args.docs < 1:
        raise SystemExit("error: --reps and --docs must be positive")
    if not (1 <= args.dimensions <= 4096):
        raise SystemExit("error: --dimensions must be 1..4096")
    if not (1 <= args.top_k <= 100):
        raise SystemExit("error: --top-k must be 1..100")

    work = Path(tempfile.mkdtemp(prefix="vantyx-rag-bench-", dir="/tmp/opencode"))
    corpus = work / "corpus"
    build_corpus(corpus, args.docs)
    index_path = work / "index.json"

    index_command = [
        str(binary),
        "ai",
        "index",
        "--root",
        str(corpus),
        "--out",
        str(index_path),
        "--provider",
        "hash",
        "--dimensions",
        str(args.dimensions),
        "--mode",
        "semantic",
        "--json",
    ]
    search_command = [
        str(binary),
        "ai",
        "search",
        "--index",
        str(index_path),
        "--query",
        QUERY,
        "--mode",
        "hybrid",
        "--top-k",
        str(args.top_k),
        "--json",
    ]

    # Warm-up (also validates the exact command path before measured runs).
    index_info, _ = run_json(index_command, timeout=300.0)
    search_info, _ = run_json(search_command, timeout=300.0)
    if index_info.get("chunks", 0) <= 0:
        raise SystemExit("error: benchmark index contains no chunks")
    if search_info.get("hits", []):
        first = search_info["hits"][0]
        if not first.get("source") or not first.get("start_line"):
            raise SystemExit("error: benchmark hit lacks a citation")

    index_times: list[float] = []
    search_times: list[float] = []
    hit_counts: list[int] = []
    for _ in range(args.reps):
        index_info, elapsed = run_json(index_command, timeout=300.0)
        index_times.append(elapsed)
        search_info, elapsed = run_json(search_command, timeout=300.0)
        search_times.append(elapsed)
        hit_counts.append(len(search_info.get("hits", [])))

    index_bytes = index_path.stat().st_size
    documents = index_info.get("documents", 0)
    chunks = index_info.get("chunks", 0)
    vectors = index_info.get("vectors", 0)
    top_k = args.top_k
    expected_hits = min(top_k, chunks)
    if any(count != expected_hits for count in hit_counts):
        raise SystemExit(
            f"error: expected {expected_hits} hits per query, observed {hit_counts}"
        )

    # Incremental updates run against the final full-build index. The corpus is
    # still walked and chunked; only embedding calls for unchanged chunks are
    # avoided. These phases are measured after the full-build numbers.
    incremental_command = index_command[:-1] + ["--incremental", "--json"]
    noop_info, _ = run_json(incremental_command, timeout=300.0)
    if noop_info.get("embedded_chunks") != 0:
        raise SystemExit(
            "error: a no-op incremental update embedded chunks: "
            f"{noop_info.get('embedded_chunks')}"
        )
    if noop_info.get("reused_chunks") != chunks:
        raise SystemExit(
            "error: a no-op incremental update did not reuse every chunk: "
            f"{noop_info.get('reused_chunks')} of {chunks}"
        )
    noop_times: list[float] = []
    noop_reused: list[int] = []
    for _ in range(args.reps):
        noop_info, elapsed = run_json(incremental_command, timeout=300.0)
        noop_times.append(elapsed)
        noop_reused.append(noop_info.get("reused_chunks", 0))
    if any(count != chunks for count in noop_reused):
        raise SystemExit(
            f"error: no-op reuse counts were inconsistent: {noop_reused}"
        )

    def append_marker(path: Path, marker: str) -> None:
        with path.open("a", encoding="utf-8") as handle:
            handle.write(f"Incremental marker {marker}.\n")

    append_marker(corpus / "doc-0000.md", "warmup")
    warm_info, _ = run_json(incremental_command, timeout=300.0)
    if warm_info.get("embedded_chunks", 0) < 1:
        raise SystemExit("error: a one-chunk update embedded nothing")
    one_times: list[float] = []
    one_embedded: list[int] = []
    one_reused: list[int] = []
    for rep in range(args.reps):
        doc = corpus / f"doc-{(rep + 1) % args.docs:04d}.md"
        append_marker(doc, f"rep-{rep}")
        info, elapsed = run_json(incremental_command, timeout=300.0)
        embedded = info.get("embedded_chunks", 0)
        if embedded < 1:
            raise SystemExit("error: a one-chunk update embedded nothing")
        if embedded > 3:
            raise SystemExit(
                "error: appending one line re-embedded too many chunks: "
                f"{embedded}"
            )
        one_times.append(elapsed)
        one_embedded.append(embedded)
        one_reused.append(info.get("reused_chunks", 0))

    generated = datetime.now(timezone.utc).isoformat(timespec="seconds")
    report = {
        "generated": generated,
        "kind": "local-rag-flat-index",
        "caveat": (
            "Offline hash embeddings and flat linear search. This measures "
            "plumbing, not semantic quality, model latency, or vector-database "
            "performance. Incremental updates still scan and chunk the corpus; "
            "they only avoid re-embedding unchanged chunks."
        ),
        "environment": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "binary": str(binary),
            "reps": args.reps,
            "dimensions": args.dimensions,
            "top_k": top_k,
            "query": QUERY,
        },
        "corpus": {
            "documents": documents,
            "chunks": chunks,
            "vectors": vectors,
            "source_bytes": sum(path.stat().st_size for path in corpus.rglob("*.md")),
            "index_bytes": index_bytes,
        },
        "index": stats(index_times),
        "search": stats(search_times),
        "incremental": {
            "noop": {
                **stats(noop_times),
                "reused_chunks": int(statistics.median(noop_reused)),
                "embedded_chunks": 0,
            },
            "one_chunk": {
                **stats(one_times),
                "reused_chunks": int(statistics.median(one_reused)),
                "embedded_chunks": int(statistics.median(one_embedded)),
            },
        },
        "hits": {
            "count": hit_counts[0],
            "first_source": search_info["hits"][0]["source"] if search_info["hits"] else None,
            "first_line": search_info["hits"][0]["start_line"] if search_info["hits"] else None,
        },
    }

    results = RESULTS
    results.mkdir(parents=True, exist_ok=True)
    out = Path(args.out) if args.out else results / f"{generated[:10]}-rag.json"
    out.write_text(json.dumps(report, indent=2) + "\n")
    markdown = out.with_suffix(".md")
    markdown.write_text(
        "\n".join(
            [
                "# Vantyx local RAG benchmark",
                "",
                f"Generated: {generated}",
                "",
                report["caveat"],
                "",
                f"Environment: {report['environment']['platform']}, "
                f"{report['environment']['machine']}, Python {report['environment']['python']}.",
                "",
                f"Corpus: {documents} Markdown documents, {chunks} chunks, "
                f"{report['corpus']['source_bytes']} source bytes, "
                f"{index_bytes} index bytes.",
                "",
                f"Index provider: hash, {args.dimensions} dimensions. "
                f"Search mode: hybrid, top-k {top_k}. Query: `{QUERY}`.",
                "",
                "| Phase | Median ms | Min ms | Max ms |",
                "| --- | ---: | ---: | ---: |",
                f"| Index (ingest + chunk + embed + save) | {report['index']['median_ms']} | "
                f"{report['index']['min_ms']} | {report['index']['max_ms']} |",
                f"| Search (load + query embed + rank) | {report['search']['median_ms']} | "
                f"{report['search']['min_ms']} | {report['search']['max_ms']} |",
                f"| Incremental, no changes (reuse {report['incremental']['noop']['reused_chunks']}) | "
                f"{report['incremental']['noop']['median_ms']} | "
                f"{report['incremental']['noop']['min_ms']} | "
                f"{report['incremental']['noop']['max_ms']} |",
                f"| Incremental, one changed chunk (embed {report['incremental']['one_chunk']['embedded_chunks']}) | "
                f"{report['incremental']['one_chunk']['median_ms']} | "
                f"{report['incremental']['one_chunk']['min_ms']} | "
                f"{report['incremental']['one_chunk']['max_ms']} |",
                "",
                f"Every measured search returned {report['hits']['count']} cited hits; "
                f"the first was `{report['hits']['first_source']}:{report['hits']['first_line']}`.",
                "",
                "One warm-up pair was discarded. Index and search are separate "
                "processes, so search time includes JSON index loading. Incremental "
                "updates still walk and chunk the corpus and include process startup, "
                "JSON load, and atomic save; the measured saving is embedding calls "
                "for unchanged chunks. This is a single-machine plumbing measurement, "
                "not a quality or model-latency claim.",
                "",
            ]
        )
    )
    print(json.dumps(report, indent=2))
    print(f"wrote {out} and {markdown}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
