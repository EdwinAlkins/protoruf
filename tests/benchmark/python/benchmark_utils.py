"""
Shared utilities for protoruf Python benchmarks.

Methodology (applied by all benchmark scripts):
  - warmup iterations before any timed run
  - GC disabled during measured runs
  - multiple runs; report median, p95, and stddev
  - msg/s and MB/s (when payload size is known)
  - optional peak traced memory (tracemalloc) and RSS delta (Linux /proc)
  - system info printed at startup (CPU, OS, Python, Rust, protobuf)
"""

from __future__ import annotations

import gc
import json
import platform
import shutil
import statistics
import subprocess
import sys
import time
import tracemalloc
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

try:
    from google.protobuf import descriptor_pool, message_factory
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    HAS_GOOGLE_PROTOBUF = True
except ImportError:
    HAS_GOOGLE_PROTOBUF = False

# Default timing parameters (overridden per benchmark where needed).
DEFAULT_WARMUP_ITERATIONS = 1_000
DEFAULT_MEASURED_RUNS = 20


@dataclass(frozen=True)
class BenchmarkStats:
    """Aggregated timing and memory metrics for one benchmark scenario."""

    median_s: float
    p95_s: float
    stddev_s: float
    msg_per_s: float
    mb_per_s: float | None
    peak_traced_bytes: int | None
    rss_delta_kb: int | None


def percentile(values: list[float], pct: float) -> float:
    """Return the `pct`-th percentile (0–100) of `values`."""
    if not values:
        raise ValueError("percentile() requires at least one value")
    if len(values) == 1:
        return values[0]
    ordered = sorted(values)
    rank = (len(ordered) - 1) * pct / 100.0
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * weight


def read_rss_kb() -> int | None:
    """Read current RSS from /proc/self/status (Linux only)."""
    status_path = Path("/proc/self/status")
    if not status_path.exists():
        return None
    for line in status_path.read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    return None


def format_bytes(num_bytes: int) -> str:
    if num_bytes >= 1_000_000:
        return f"{num_bytes / 1_000_000:.1f} MB"
    if num_bytes >= 1_000:
        return f"{num_bytes / 1_000:.1f} KB"
    return f"{num_bytes} B"


def get_protobuf_version() -> str:
    if not HAS_GOOGLE_PROTOBUF:
        return "not installed"
    try:
        import google.protobuf

        return google.protobuf.__version__
    except Exception:
        return "unknown"


def get_rustc_version() -> str:
    rustc = shutil.which("rustc")
    if rustc is None:
        return "not found"
    try:
        out = subprocess.run(
            [rustc, "--version"],
            check=True,
            capture_output=True,
            text=True,
        )
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def print_system_info() -> None:
    print("System")
    print(f"  CPU:      {platform.processor() or 'unknown'} ({platform.machine()})")
    print(f"  OS:       {platform.system()} {platform.release()}")
    print(f"  Python:   {platform.python_version()} ({sys.implementation.name})")
    print(f"  Rust:     {get_rustc_version()}")
    print(f"  protobuf: {get_protobuf_version()}")
    print()


def print_methodology(*, warmup: int, runs: int, iterations: int) -> None:
    print("Methodology")
    print("  - release build of protoruf (see README)")
    print(f"  - {warmup:,} warmup iteration(s) per scenario")
    print(f"  - {runs} measured run(s); median, p95, and stddev reported")
    print("  - GC disabled during measured runs")
    print(f"  - each measured run executes {iterations:,} conversion(s)")
    print(
        "  - measures the full JSON ↔ Protobuf conversion stack "
        "(JSON parse + encode/decode + JSON emit), not protobuf encode alone"
    )
    print()


def warn_missing_google_protobuf() -> None:
    if HAS_GOOGLE_PROTOBUF:
        return
    print("⚠️  google.protobuf not installed, only protoruf will be measured.")
    print("   Install it with: uv sync --group benchmark\n")


def create_google_message_factory(descriptor_bytes: bytes, message_type: str):
    """
    Build a google.protobuf message class from protoruf's FileDescriptorSet.

    Returns a callable that creates a fresh message instance.
    """
    fds = FileDescriptorSet()
    fds.ParseFromString(descriptor_bytes)

    pool = descriptor_pool.DescriptorPool()
    for file_proto in fds.file:
        pool.Add(file_proto)

    msg_desc = pool.FindMessageTypeByName(message_type)

    if hasattr(message_factory, "GetMessageClass"):
        message_class = message_factory.GetMessageClass(msg_desc)
    else:
        factory = message_factory.MessageFactory(pool=pool)
        if hasattr(factory, "GetMessageClass"):
            message_class = factory.GetMessageClass(msg_desc)
        else:
            message_class = factory.GetPrototype(msg_desc)

    return message_class


def make_record(index: int) -> dict:
    return {
        "id": f"rec-{index:08d}",
        "name": f"Record number {index}",
        # STATUS_ACTIVE / STATUS_ARCHIVED — non-zero on purpose: proto3
        # omits default (0) values, which would drop the field on output.
        "status": (index % 2) + 1,
        "timestamp": 1_700_000_000 + index,
        "tags": [f"tag-{index % 50}", f"group-{index % 10}"],
        "attributes": {"region": "eu-west", "tier": str(index % 5), "note": "x" * 16},
        "embedding": {
            "values": [round((index % 7) * 0.125 + k, 4) for k in range(8)],
        },
    }


def build_large_dataset(n_records: int) -> dict:
    """Build a deterministic `bench.Dataset` with exactly `n_records` records."""
    return {
        "dataset_id": "ds-benchmark-1",
        "description": "large benchmark dataset",
        "records": [make_record(i) for i in range(n_records)],
        "metadata": {"source": "benchmark", "version": "1.0", "env": "bench"},
    }


def _log_progress(message: str, *, end: str = "\n") -> None:
    print(message, end=end, flush=True)


def run_timed_benchmark(
    loop_body: Callable[[], None],
    *,
    iterations: int,
    payload_bytes: int,
    warmup_iterations: int = DEFAULT_WARMUP_ITERATIONS,
    measured_runs: int = DEFAULT_MEASURED_RUNS,
    label: str = "benchmark",
) -> BenchmarkStats:
    """
    Warm up, then time `loop_body` several times with GC disabled.

    `loop_body` should perform exactly `iterations` conversions.
    Progress is printed to stdout after warmup, each measured run, and memory sampling.
    """
    prefix = f"  [{label}]"

    _log_progress(f"{prefix} warmup ({warmup_iterations:,} pass(es))...", end="")
    warmup_start = time.perf_counter()
    for _ in range(warmup_iterations):
        loop_body()
    _log_progress(f" done ({time.perf_counter() - warmup_start:.2f}s)")

    gc_was_enabled = gc.isenabled()
    gc.disable()
    try:
        runs: list[float] = []
        for run_idx in range(1, measured_runs + 1):
            start = time.perf_counter()
            loop_body()
            elapsed = time.perf_counter() - start
            runs.append(elapsed)
            instant_msg_s = iterations / elapsed
            line = f"{prefix} run {run_idx:>2}/{measured_runs} — {elapsed:.4f}s ({instant_msg_s:,.0f} msg/s)"
            if payload_bytes >= 1_000:
                instant_mb_s = (payload_bytes / 1_000_000) * instant_msg_s
                line += f"   {instant_mb_s:,.0f} MB/s"
            _log_progress(line)
    finally:
        if gc_was_enabled:
            gc.enable()

    median_s = statistics.median(runs)
    stddev_s = statistics.stdev(runs) if len(runs) > 1 else 0.0
    p95_s = percentile(runs, 95)
    msg_per_s = iterations / median_s
    mb_per_s = (payload_bytes / 1_000_000) * msg_per_s

    _log_progress(f"{prefix} measuring memory...", end="")
    peak_traced, rss_delta = measure_memory(loop_body)
    _log_progress(" done")

    return BenchmarkStats(
        median_s=median_s,
        p95_s=p95_s,
        stddev_s=stddev_s,
        msg_per_s=msg_per_s,
        mb_per_s=mb_per_s,
        peak_traced_bytes=peak_traced,
        rss_delta_kb=rss_delta,
    )


def measure_memory(loop_body: Callable[[], None]) -> tuple[int | None, int | None]:
    """Run `loop_body` once and report peak traced Python memory and RSS delta."""
    gc.collect()
    rss_before = read_rss_kb()

    tracemalloc.start()
    loop_body()
    _, peak_traced = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    rss_after = read_rss_kb()
    rss_delta = None
    if rss_before is not None and rss_after is not None:
        rss_delta = max(rss_after - rss_before, 0)

    return peak_traced, rss_delta


def _stats_parts(stats: BenchmarkStats, *, show_mb_s: bool) -> list[str]:
    parts = [
        f"median {stats.median_s:.4f}s",
        f"p95 {stats.p95_s:.4f}s",
        f"σ {stats.stddev_s:.4f}s",
        f"{stats.msg_per_s:,.0f} msg/s",
    ]
    if show_mb_s and stats.mb_per_s is not None:
        parts.append(f"{stats.mb_per_s:,.0f} MB/s")
    if stats.peak_traced_bytes is not None:
        parts.append(f"peak traced {format_bytes(stats.peak_traced_bytes)}")
    if stats.rss_delta_kb is not None:
        parts.append(f"RSS Δ {stats.rss_delta_kb / 1_024:.1f} MB")
    return parts


def format_stats_line(stats: BenchmarkStats, *, show_mb_s: bool) -> str:
    return "  → " + "   ".join(_stats_parts(stats, show_mb_s=show_mb_s))


def print_comparison_table(
    title: str,
    rows: list[tuple[str, BenchmarkStats | None, BenchmarkStats]],
    *,
    show_mb_s: bool,
    protoruf_label: str = "protoruf",
) -> None:
    print("=" * 90)
    print(title)
    print("=" * 90)

    for label, google_stats, protoruf_stats in rows:
        print(f"\n{label}")
        if google_stats is not None:
            google_line = "   ".join(_stats_parts(google_stats, show_mb_s=show_mb_s))
            protoruf_line = "   ".join(_stats_parts(protoruf_stats, show_mb_s=show_mb_s))
            print(f"  google.protobuf: {google_line}")
            speedup = google_stats.median_s / protoruf_stats.median_s
            print(f"  {protoruf_label}:      {protoruf_line}")
            print(f"  speedup (median): {speedup:.1f}x on the JSON ↔ Protobuf conversion stack")
        else:
            print(format_stats_line(protoruf_stats, show_mb_s=show_mb_s))

    print(
        "\nNote: speedup compares the full JSON ↔ Protobuf conversion path "
        "(including JSON parsing/formatting), not raw protobuf encoding alone."
    )
