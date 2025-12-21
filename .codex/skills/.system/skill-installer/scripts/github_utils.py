#!/usr/bin/env python3
"""Shared GitHub helpers for skill install scripts."""

from __future__ import annotations

import os
import re
import urllib.parse
import urllib.request

REPO_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


def github_request(url: str, user_agent: str) -> bytes:
    if not url.startswith("https://"):
        raise ValueError("Only HTTPS URLs are supported")
    headers = {"User-Agent": user_agent}
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"token {token}"
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req) as resp:
        return resp.read()


def github_api_contents_url(repo: str, path: str, ref: str) -> str:
    if not REPO_PATTERN.match(repo):
        raise ValueError("Repo must be in the form 'owner/repo'.")
    clean_path = path.strip("/")
    safe_path = urllib.parse.quote(clean_path, safe="/")
    safe_ref = urllib.parse.quote(ref, safe="")
    return f"https://api.github.com/repos/{repo}/contents/{safe_path}?ref={safe_ref}"
