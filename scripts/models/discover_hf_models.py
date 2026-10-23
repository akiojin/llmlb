#!/usr/bin/env python3
"""Discover HF model candidates; publish only when explicitly requested."""

import argparse
from datetime import datetime, timedelta, timezone
from http.client import HTTPException
import json
import math
import os
from pathlib import Path
import re
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[2]
ISSUE_MARKER = "<!-- llmlb:hf-model-discovery -->"
ISSUE_TITLE = "[model-discovery] Hugging Face 新モデル候補"
TEXT_TASKS = {"text-generation", "text2text-generation", "image-text-to-text"}
MODEL_ID = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")


def known_models(source):
    """Read only the static mapping table, including legacy engine aliases."""
    source = re.sub(r"//[^\n]*|/\*.*?\*/", "", source, flags=re.S)
    table = re.search(r"pub static BUILTIN_MAPPINGS[^=]*=\s*&\[(.*?)^\];",
                      source, re.S | re.M)
    if not table:
        raise ValueError("BUILTIN_MAPPINGS table is missing or malformed")
    canonical = re.findall(r'\bcanonical\s*:\s*"([^"\n]+)"', table[1])
    if not canonical or len(canonical) != len(re.findall(r"\bModelMapping\s*\{", table[1])):
        raise ValueError("BUILTIN_MAPPINGS canonical entries cannot be verified")
    aliases = re.findall(r'\bname\s*:\s*"([^"\n]+)"', table[1])
    return {name.lower() for name in canonical + aliases}


def published_at(model):
    value = model.get("createdAt")
    if not isinstance(value, str):
        return None
    try:
        date = datetime.fromisoformat(value.replace("Z", "+00:00"))
        return date.astimezone(timezone.utc) if date.tzinfo else None
    except ValueError:
        return None


def parameter_count(model):
    totals = []
    for key in ("safetensors", "gguf"):
        metadata = model.get(key)
        total = metadata.get("total") if isinstance(metadata, dict) else None
        if type(total) in (int, float) and math.isfinite(total) and total > 0:
            totals.append(total)
    # A GGUF repository can carry a small safetensors sidecar as well.
    return max(totals) if totals else None


def select_candidates(models, known):
    """Require explicit task and total parameter metadata; never guess MoE size."""
    candidates = []
    for model in models:
        model_id = model.get("id", "")
        date = published_at(model)
        total = parameter_count(model)
        if (not isinstance(model_id, str) or not MODEL_ID.fullmatch(model_id)
                or model_id.lower() in known or model.get("pipeline_tag") not in TEXT_TASKS
                or date is None or total is None or total >= 120_000_000_000):
            continue
        candidates.append({"id": model_id, "published": date.date().isoformat(),
                           "parameters": total, "trending_rank": model.get("trending_rank")})
    return sorted(candidates, key=lambda row: (row["trending_rank"] or math.inf, row["id"]))


def request_json(url, token=None, method="GET", body=None):
    headers = {"Accept": "application/json", "User-Agent": "llmlb-model-discovery"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    payload = None if body is None else json.dumps(body).encode()
    if payload is not None:
        headers["Content-Type"] = "application/json"
    request = Request(url, data=payload, headers=headers, method=method)
    with urlopen(request, timeout=30) as response:
        data = json.load(response)
        link = response.headers.get("Link", "")
    next_link = re.search(r'<([^>]+)>;\s*rel="next"', link)
    return data, next_link[1] if next_link else None


def hf_page(url):
    # Do not send the optional HF token to an arbitrary pagination origin.
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.netloc != "huggingface.co":
        raise ValueError("unexpected Hugging Face pagination origin")
    return request_json(url, os.environ.get("HF_TOKEN"))


def discover(config, known, now, fetch=hf_page):
    """Keep successful data and represent fetch failures separately from zero results."""
    cutoff = now - timedelta(days=config["recent_days"])
    models, warnings = {}, []
    sources = [(f"author:{author}", {"author": author, "sort": "createdAt"})
               for author in config["authors"]]
    sources.append(("trending", {"sort": "trendingScore"}))
    for source, query in sources:
        limit = config["trending_limit"] if source == "trending" else config["page_size"]
        query.update({"direction": -1, "limit": min(limit, config["page_size"]),
                      "expand[]": ["createdAt", "pipeline_tag", "safetensors", "gguf"]})
        url = "https://huggingface.co/api/models?" + urlencode(query, doseq=True)
        seen, rank = set(), 0
        try:
            while url:
                if url in seen:
                    raise ValueError("repeated pagination link")
                seen.add(url)
                rows, next_url = fetch(url)
                if not isinstance(rows, list):
                    raise ValueError("expected a model list")
                stop = False
                for row in rows:
                    if (not isinstance(row, dict) or not isinstance(row.get("id"), str)
                            or not MODEL_ID.fullmatch(row["id"]) or published_at(row) is None):
                        raise ValueError("model ID or publication date is missing/malformed")
                    if row.get("pipeline_tag") is not None and not isinstance(row["pipeline_tag"], str):
                        raise ValueError("model task metadata is malformed")
                    rank += 1
                    if source != "trending" and published_at(row) < cutoff:
                        stop = True
                        continue
                    key = row["id"].lower()
                    # Preserve publisher metadata if the same model also trends.
                    models.setdefault(key, dict(row))
                    if source == "trending":
                        models[key]["trending_rank"] = rank
                        if rank >= limit:
                            stop = True
                            break
                url = None if stop else next_url
        except (OSError, ValueError, HTTPException) as error:
            warnings.append(f"{source}: {error}")
    return {"status": "incomplete" if warnings else "complete", "warnings": warnings,
            "candidates": select_candidates(models.values(), known),
            "unverified_size": sorted(row["id"] for row in models.values()
                                      if row.get("pipeline_tag") in TEXT_TASKS
                                      and parameter_count(row) is None)}


def github_api(method, path, body=None):
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        raise ValueError("--publish requires GH_TOKEN or GITHUB_TOKEN")
    data, _ = request_json("https://api.github.com" + path, token, method, body)
    return data


def github_pages(path, api):
    page = 1
    while True:
        separator = "&" if "?" in path else "?"
        rows = api("GET", f"{path}{separator}per_page=100&page={page}")
        if not isinstance(rows, list):
            raise ValueError("expected a GitHub list response")
        yield from rows
        if len(rows) < 100:
            break
        page += 1


def model_marker(model_id):
    return f"<!-- hf-model:{model_id} -->"


def issue_body(report, candidates):
    lines = [ISSUE_MARKER, "## Hugging Face モデル候補", "",
             f"取得状態: `{report['status']}`。対応表は自動変更していません。",
             "公開日は HF の createdAt、順位は全モデルの trendingScore 降順です。",
             "Ollama / LM Studio のタグ実在と採用可否を確認してください。", "",
             "| モデル ID | 公開日 (UTC) | トレンド順位 | 総パラメータ (B) |",
             "|---|---|---|---|"]
    for row in candidates:
        model_id = row["id"]
        rank = row["trending_rank"] or "対象外"
        lines.append(f"| [{model_id}](https://huggingface.co/{model_id}) "
                     f"{model_marker(model_id)} | {row['published']} | {rank} "
                     f"| {row['parameters'] / 1_000_000_000:.3f} |")
    if report["warnings"]:
        lines.extend(["", "取得に失敗した公開元/トレンドがあります。未取得分の候補有無は不明です。"])
    return "\n".join(lines)


def publish(report, repository, api=github_api):
    if not report["candidates"]:
        return "no candidates to publish"
    if not MODEL_ID.fullmatch(repository):
        raise ValueError("a valid owner/repository is required")
    path = f"/repos/{repository}/issues"
    issue = next((row for row in github_pages(path + "?state=open", api)
                  if "pull_request" not in row and (ISSUE_MARKER in (row.get("body") or "")
                  or row.get("title") == ISSUE_TITLE)), None)
    if issue:
        comments_path = f"{path}/{issue['number']}/comments"
        previous = "\n".join([issue.get("body") or ""] +
                             [row.get("body") or "" for row in github_pages(comments_path, api)])
        new = [row for row in report["candidates"] if model_marker(row["id"]) not in previous]
        if new:
            api("POST", comments_path, {"body": issue_body(report, new)})
        return f"existing issue #{issue['number']}: {len(new)} new candidates"
    created = api("POST", path, {"title": ISSUE_TITLE,
                                 "body": issue_body(report, report["candidates"])})
    return f"created issue #{created['number']}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path(__file__).with_name("hf-discovery-config.json"))
    parser.add_argument("--mapping", type=Path, default=ROOT / "llmlb/src/models/mapping.rs")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--publish", action="store_true", help="create/update the aggregate GitHub Issue")
    args = parser.parse_args()
    config = json.loads(args.config.read_text(encoding="utf-8"))
    if (not isinstance(config.get("authors"), list) or not config["authors"]
            or any(not isinstance(author, str) or not re.fullmatch(r"[A-Za-z0-9_.-]+", author)
                   for author in config["authors"])
            or any(type(config.get(key)) is not int or config[key] < 1
                   for key in ("recent_days", "trending_limit", "page_size"))):
        raise ValueError("invalid discovery config")
    known = known_models(args.mapping.read_text(encoding="utf-8"))
    report = discover(config, known, datetime.now(timezone.utc))
    serialized = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(serialized, encoding="utf-8")
    print(serialized, end="")
    for warning in report["warnings"]:
        escaped = warning.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::warning::{escaped}")
    if report["status"] == "incomplete":
        print("Discovery incomplete: missing data must not be interpreted as no new models.")
    if args.publish:
        print(publish(report, os.environ.get("GITHUB_REPOSITORY", "")))


if __name__ == "__main__":
    main()
