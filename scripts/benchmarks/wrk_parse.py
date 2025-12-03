#!/usr/bin/env python3
"""
Parse wrk --latency output and emit a CSV row.

Usage:
  wrk ... --latency | scripts/benchmarks/wrk_parse.py --label local \
    > benchmarks/results/20251125-local.csv
"""
import argparse
import re
import sys
from typing import Dict, Optional


PERCENTILES = ["50", "75", "90", "95", "99"]


def to_ms(value: float, unit: str) -> float:
    unit = unit.lower()
    if unit.startswith("s") and unit != "ms":
        # s or sec
        return value * 1000.0
    if unit.startswith("us"):
        return value / 1000.0
    return value  # assume ms


def parse_wrk(text: str) -> Dict[str, Optional[float]]:
    data: Dict[str, Optional[float]] = {f"p{p}": None for p in PERCENTILES}
    data.update(
        {
            "rps": None,
            "non2xx": 0,
            "socket_errors": 0,
            "requests": None,
            "duration_s": None,
        }
    )

    for line in text.splitlines():
        if "Requests/sec" in line:
            m = re.search(r"Requests/sec:\s+([0-9.]+)", line)
            if m:
                data["rps"] = float(m.group(1))
        elif "Latency Distribution" in line:
            # The next lines carry percentiles; handled below in generic matcher
            continue
        elif re.search(r"^\s*(\d+)%\s+([\d.]+)\s*([a-zA-Z]+)$", line):
            m = re.search(r"^\s*(\d+)%\s+([\d.]+)\s*([a-zA-Z]+)$", line)
            if m:
                pct, val, unit = m.groups()
                if pct in PERCENTILES:
                    data[f"p{pct}"] = to_ms(float(val), unit)
        elif "Non-2xx or 3xx responses" in line:
            m = re.search(r"Non-2xx or 3xx responses:\s+(\d+)", line)
            if m:
                data["non2xx"] = int(m.group(1))
        elif "Socket errors" in line:
            # e.g. "Socket errors: connect 0, read 0, write 0, timeout 0"
            nums = re.findall(r"\b(\d+)\b", line)
            data["socket_errors"] = sum(int(n) for n in nums)
        elif "requests in" in line and "read" in line:
            m = re.search(r"(\d+)\s+requests in\s+([\d.]+)s", line)
            if m:
                data["requests"] = int(m.group(1))
                data["duration_s"] = float(m.group(2))

    return data


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Parse wrk output and print one CSV row."
    )
    parser.add_argument(
        "--label", default="run", help="Label column (e.g., local/openai/gemini)"
    )
    args = parser.parse_args()

    text = sys.stdin.read()
    metrics = parse_wrk(text)

    header = [
        "label",
        "rps",
        "p50_ms",
        "p75_ms",
        "p90_ms",
        "p95_ms",
        "p99_ms",
        "non2xx",
        "socket_errors",
        "requests",
        "duration_s",
    ]
    row = [
        args.label,
        metrics["rps"],
        metrics["p50"],
        metrics["p75"],
        metrics["p90"],
        metrics["p95"],
        metrics["p99"],
        metrics["non2xx"],
        metrics["socket_errors"],
        metrics["requests"],
        metrics["duration_s"],
    ]

    print(",".join(str(x) if x is not None else "" for x in header))
    print(",".join("" if x is None else str(x) for x in row))


if __name__ == "__main__":
    main()
