"""Issue #885: offline acceptance tests for discovery and publication."""

import importlib.util
import json
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from http.client import IncompleteRead
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import threading
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/models/discover_hf_models.py"
NOW = datetime(2026, 10, 14, tzinfo=timezone.utc)


def model(model_id="Qwen/New-7B", total=7_000_000_000, **fields):
    return {
        "id": model_id,
        "createdAt": "2026-10-12T00:00:00.000Z",
        "pipeline_tag": "text-generation",
        "safetensors": {"total": total},
        **fields,
    }


class DiscoveryTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(SCRIPT.is_file(), "HF discovery script has not been implemented")
        spec = importlib.util.spec_from_file_location("hf_discovery", SCRIPT)
        self.mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.mod)
        self.config = {"authors": ["Qwen", "zai-org"], "recent_days": 14,
                       "trending_limit": 100, "page_size": 100}

    def test_mapping_parser_excludes_comments_and_outside_table(self):
        source = '''canonical: "outside/before",
pub static BUILTIN_MAPPINGS: &[ModelMapping] = &[
    // canonical: "comment/fake",
    ModelMapping {
        canonical: "Qwen/Known-7B",
        aliases: &[EngineAlias { engine: EndpointType::Xllm, name: "old/known" }],
    },
];
canonical: "outside/after",
'''
        self.assertEqual(self.mod.known_models(source), {"qwen/known-7b", "old/known"})

    def test_mapping_parser_rejects_missing_or_empty_table(self):
        for text in ("", "pub static BUILTIN_MAPPINGS: &[ModelMapping] = &[];"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.mod.known_models(text)

    def test_real_mapping_table_is_read(self):
        known = self.mod.known_models((ROOT / "llmlb/src/models/mapping.rs").read_text())
        self.assertIn("openai/gpt-oss-20b", known)
        self.assertGreater(len(known), 20)

    def test_selection_requires_unmapped_text_generation_under_120b(self):
        rows = [model(), model("Qwen/Known-7B"),
                model("org/exact-120B", 120_000_000_000),
                model("org/large-235B-A22B", 235_000_000_000),
                model("org/image", pipeline_tag="text-to-image"),
                model("org/embed", pipeline_tag="feature-extraction"),
                model("org/unknown", None), model("org/zero", 0),
                model("org/small", 119_999_999_999)]
        selected = self.mod.select_candidates(rows, {"qwen/known-7b"})
        self.assertEqual({r["id"] for r in selected}, {"Qwen/New-7B", "org/small"})

    def test_alias_case_insensitive_exact_match(self):
        selected = self.mod.select_candidates(
            [model("OLD/KNOWN"), model("old/known-extra")], {"old/known"})
        self.assertEqual([r["id"] for r in selected], ["old/known-extra"])

    def test_unknown_size_does_not_use_active_parameter_name(self):
        rows = [model("org/MoE-A3B", None), model("org/bad", True),
                model("org/bad-string", "7B"), model("org/bad-negative", -7),
                model("org/gguf", None, gguf={"total": 8_000_000_000}),
                model("org/vlm", pipeline_tag="image-text-to-text")]
        self.assertEqual({r["id"] for r in self.mod.select_candidates(rows, set())},
                         {"org/gguf", "org/vlm"})

    def test_gguf_total_cannot_be_hidden_by_small_safetensors_sidecar(self):
        rows = [model("org/MoE-235B", 3_000_000_000, gguf={"total": 235_000_000_000})]
        self.assertEqual(self.mod.select_candidates(rows, set()), [])

    def test_malformed_dates_and_unsafe_ids_are_excluded(self):
        rows = [model("org/valid"), model("org/bad-date", createdAt="bad"),
                model("org/line\n::error::oops"), model("org/undated", createdAt=None)]
        self.assertEqual([r["id"] for r in self.mod.select_candidates(rows, set())],
                         ["org/valid"])

    def test_recent_publisher_pagination_and_global_trend_ranks(self):
        calls = []

        def fetch(url):
            calls.append(url)
            query = parse_qs(urlparse(url).query)
            if query.get("sort") == ["trendingScore"]:
                return [model("org/image", pipeline_tag="text-to-image"),
                        model(createdAt="2025-01-01T00:00:00Z")], None
            if query.get("author") == ["Qwen"]:
                return [model()], "https://huggingface.co/api/models?cursor=next"
            if query.get("cursor") == ["next"]:
                return [model("Qwen/Page2-8B"),
                        model("Qwen/old", createdAt="2025-01-01T00:00:00Z")], None
            return [], None

        report = self.mod.discover(self.config, set(), NOW, fetch)
        self.assertEqual(report["status"], "complete")
        self.assertEqual({r["id"] for r in report["candidates"]},
                         {"Qwen/New-7B", "Qwen/Page2-8B"})
        merged = next(r for r in report["candidates"] if r["id"] == "Qwen/New-7B")
        self.assertEqual(merged["trending_rank"], 2)
        trend_query = parse_qs(urlparse(calls[-1]).query)
        self.assertEqual(trend_query["limit"], ["100"])
        self.assertIn("safetensors", trend_query["expand[]"])
        self.assertEqual(len(calls), 4)

    def test_hf_failure_is_incomplete_not_no_new_models(self):
        def fetch(url):
            raise OSError("HF unavailable")

        report = self.mod.discover(self.config, set(), NOW, fetch)
        self.assertEqual(report["status"], "incomplete")
        self.assertEqual(report["candidates"], [])
        self.assertEqual(len(report["warnings"]), 3)

    def test_partial_failure_preserves_successful_candidates(self):
        def fetch(url):
            if "trendingScore" in url:
                return [model("org/trend")], None
            raise OSError("rate limited")

        report = self.mod.discover(self.config, set(), NOW, fetch)
        self.assertEqual(report["status"], "incomplete")
        self.assertEqual([r["id"] for r in report["candidates"]], ["org/trend"])

    def test_truncated_hf_transfer_is_incomplete(self):
        def fetch(url):
            raise IncompleteRead(b"partial", 100)

        report = self.mod.discover(self.config, set(), NOW, fetch)
        self.assertEqual(report["status"], "incomplete")
        self.assertEqual(len(report["warnings"]), 3)

    def test_malformed_hf_response_is_incomplete(self):
        for data in ({"error": "upstream"}, [None], [{"id": "org/no-date"}]):
            with self.subTest(data=data):
                report = self.mod.discover(self.config, set(), NOW, lambda url: (data, None))
                self.assertEqual(report["status"], "incomplete")

    def test_successful_empty_response_is_complete(self):
        report = self.mod.discover(self.config, set(), NOW, lambda url: ([], None))
        self.assertEqual(report["status"], "complete")
        self.assertEqual(report["warnings"], [])

    def test_http_transport_pagination_and_failure_are_measured(self):
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path == "/fail":
                    self.send_error(503)
                    return
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Link", '<https://huggingface.co/api/models?cursor=2>; rel="next"')
                self.end_headers()
                self.wfile.write(json.dumps([model()]).encode())

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            url = f"http://127.0.0.1:{server.server_port}"
            rows, next_url = self.mod.request_json(url)
            self.assertEqual(rows[0]["id"], "Qwen/New-7B")
            self.assertEqual(next_url, "https://huggingface.co/api/models?cursor=2")
            report = self.mod.discover(self.config, set(), NOW,
                                       lambda ignored: self.mod.request_json(url + "/fail"))
            self.assertEqual(report["status"], "incomplete")
            self.assertTrue(all("503" in warning for warning in report["warnings"]))
        finally:
            server.shutdown()
            server.server_close()
            worker.join()

    def test_cli_hf_failure_exits_zero_and_saves_incomplete_artifact(self):
        code = '''import runpy, sys
from unittest.mock import patch
from urllib.error import URLError
from http.client import IncompleteRead
path, output, failure = sys.argv[1:]
sys.argv = [path, "--output", output]
error = URLError("HF unavailable") if failure == "unavailable" else IncompleteRead(b"partial", 100)
with patch("urllib.request.urlopen", side_effect=error):
    runpy.run_path(path, run_name="__main__")
'''
        with TemporaryDirectory() as directory:
            output = Path(directory) / "report.json"
            for failure in ("unavailable", "truncated"):
                with self.subTest(failure=failure):
                    result = subprocess.run([sys.executable, "-c", code, str(SCRIPT),
                                             str(output), failure],
                                            capture_output=True, text=True, timeout=10)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(output.read_text())["status"], "incomplete")
                    self.assertIn("::warning::", result.stdout)
                    self.assertIn("must not be interpreted as no new models", result.stdout)

    def test_hf_token_is_never_sent_to_foreign_pagination_origin(self):
        with patch.object(self.mod, "request_json") as request:
            with self.assertRaises(ValueError):
                self.mod.hf_page("https://attacker.invalid/?cursor=1")
            request.assert_not_called()

    def test_malformed_task_metadata_is_incomplete_instead_of_crashing(self):
        report = self.mod.discover(self.config, set(), NOW,
                                   lambda url: ([model(pipeline_tag=[])], None))
        self.assertEqual(report["status"], "incomplete")

    def test_config_tracks_required_publishers(self):
        config = json.loads((ROOT / "scripts/models/hf-discovery-config.json").read_text())
        self.assertTrue(set(["Qwen", "zai-org", "google", "nvidia", "meta-llama",
                             "moonshotai", "mistralai", "deepseek-ai", "openai",
                             "microsoft"]).issubset(config["authors"]))
        self.assertEqual(config["trending_limit"], 100)

    def test_publish_creates_one_issue_with_model_date_and_rank(self):
        report = {"status": "complete", "warnings": [],
                  "candidates": self.mod.select_candidates([model(trending_rank=3)], set())}
        writes = []

        def api(method, path, body=None):
            if method == "GET":
                return []
            writes.append((path, body))
            return {"number": 12}

        self.mod.publish(report, "owner/repo", api)
        self.assertEqual(len(writes), 1)
        self.assertEqual(writes[0][0], "/repos/owner/repo/issues")
        body = writes[0][1]["body"]
        for text in ("Qwen/New-7B", "2026-10-12", "3", self.mod.ISSUE_MARKER):
            self.assertIn(text, body)

    def test_publish_finds_open_issue_on_second_page_and_appends(self):
        report = {"status": "incomplete", "warnings": ["publisher unavailable"],
                  "candidates": self.mod.select_candidates([model()], set())}
        writes = []

        def api(method, path, body=None):
            if method == "POST":
                writes.append((path, body))
                return {}
            if "/comments" in path:
                return []
            if parse_qs(urlparse(path).query).get("page") == ["1"]:
                return [{"number": i, "title": "unrelated", "body": ""} for i in range(100)]
            return [{"number": 42, "title": "changed", "body": self.mod.ISSUE_MARKER}]

        self.mod.publish(report, "owner/repo", api)
        self.assertEqual([path for path, body in writes], ["/repos/owner/repo/issues/42/comments"])
        self.assertIn("incomplete", writes[0][1]["body"])

    def test_publish_skips_models_already_mentioned_in_comments(self):
        report = {"status": "complete", "warnings": [],
                  "candidates": self.mod.select_candidates([model()], set())}
        writes = []

        def api(method, path, body=None):
            if method == "POST":
                writes.append(body)
            if "/comments" in path:
                return [{"body": "<!-- hf-model:Qwen/New-7B -->"}]
            return [{"number": 42, "title": self.mod.ISSUE_TITLE, "body": ""}]

        self.mod.publish(report, "owner/repo", api)
        self.assertEqual(writes, [])

    def test_no_candidate_never_creates_issue_even_when_incomplete(self):
        def api(*args):
            self.fail("GitHub must not be contacted without candidates")

        self.mod.publish({"status": "incomplete", "warnings": ["HF down"],
                          "candidates": []}, "owner/repo", api)

    def test_matching_pull_request_does_not_prevent_issue_creation(self):
        report = {"status": "complete", "warnings": [],
                  "candidates": self.mod.select_candidates([model()], set())}
        writes = []

        def api(method, path, body=None):
            if method == "GET":
                return [{"number": 2, "title": self.mod.ISSUE_TITLE,
                         "body": self.mod.ISSUE_MARKER, "pull_request": {}}]
            writes.append(path)
            return {"number": 3}

        self.mod.publish(report, "owner/repo", api)
        self.assertEqual(writes, ["/repos/owner/repo/issues"])


if __name__ == "__main__":
    unittest.main()
