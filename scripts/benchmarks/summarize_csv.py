#!/usr/bin/env python3
"""
Summarize one or more wrk CSV files into a markdown table.

Usage:
  scripts/benchmarks/summarize_csv.py benchmarks/results/*.csv
"""
import csv
import sys
from pathlib import Path
from typing import List, Dict, Any


def load_csv(path: Path) -> Dict[str, Any]:
    with path.open() as f:
        reader = csv.DictReader(f)
        rows = list(reader)
        if not rows:
            raise ValueError(f"No rows in {path}")
        row = rows[0]
        row["source"] = str(path)
        return row


def to_float(val: str) -> str:
    try:
        return f"{float(val):.2f}"
    except Exception:
        return ""


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: summarize_csv.py <csv> [<csv> ...]", file=sys.stderr)
        sys.exit(1)

    rows: List[Dict[str, Any]] = []
    for arg in sys.argv[1:]:
        rows.append(load_csv(Path(arg)))

    # header order
    headers = ["label", "rps", "p95_ms", "p99_ms", "non2xx", "socket_errors", "requests", "duration_s", "source"]
    print("| " + " | ".join(headers) + " |")
    print("|" + "|".join(["---"] * len(headers)) + "|")
    for row in rows:
        print(
            "| "
            + " | ".join(
                [
                    row.get("label", ""),
                    to_float(row.get("rps", "")),
                    to_float(row.get("p95_ms", "")),
                    to_float(row.get("p99_ms", "")),
                    row.get("non2xx", ""),
                    row.get("socket_errors", ""),
                    row.get("requests", ""),
                    to_float(row.get("duration_s", "")),
                    row.get("source", ""),
                ]
            )
            + " |"
        )


if __name__ == "__main__":
    main()
