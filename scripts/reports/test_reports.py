"""Checks for the report intake and the transcription step.

  python3 -m unittest scripts/reports/test_reports.py
"""

from __future__ import annotations

import base64
import json
import os
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).parent))

import intake  # noqa: E402
import transcribe  # noqa: E402


def report(**overrides) -> dict:
    value = {
        "schema": "meshy-dcc.report.v1",
        "source": "cli",
        "summary": "Importing a GLB into Blender froze the scene",
        "client": {"version": "0.2.1", "os": "macos", "arch": "aarch64"},
        "environment": {},
        "findings": [],
    }
    value.update(overrides)
    return value


class FakeS3:
    def __init__(self):
        self.objects = {}

    def put_object(self, Bucket, Key, Body, ContentType):
        self.objects[(Bucket, Key)] = json.loads(Body)


def post(body: bytes, method: str = "POST", base64_encoded: bool = False) -> dict:
    return {
        "requestContext": {"http": {"method": method}},
        "body": base64.b64encode(body).decode() if base64_encoded else body.decode(),
        "isBase64Encoded": base64_encoded,
    }


class IntakeTest(unittest.TestCase):
    def setUp(self):
        self.s3 = FakeS3()
        intake._s3 = self.s3
        os.environ["REPORTS_BUCKET"] = "reports"

    def test_valid_report_is_stored_under_pending(self):
        response = intake.handler(post(json.dumps(report()).encode(), base64_encoded=True), None)
        self.assertEqual(response["statusCode"], 202)
        report_id = json.loads(response["body"])["report_id"]
        stored = self.s3.objects[("reports", f"pending/{report_id}.json")]
        self.assertEqual(stored["report"]["summary"], report()["summary"])

    def test_rejections_store_nothing(self):
        cases = [
            (post(b"{}", method="GET"), 405),
            (post(b"x" * (intake.MAX_BYTES + 1)), 413),
            (post(b"not json"), 400),
            (post(json.dumps(report(schema="other")).encode()), 400),
            (post(json.dumps(report(summary="  ")).encode()), 400),
            (post(json.dumps(report(source="web")).encode()), 400),
            (post(json.dumps(report(client={"version": "0.2.1"})).encode()), 400),
            (post(json.dumps(report(findings=[{}] * 21)).encode()), 400),
        ]
        for event, status in cases:
            self.assertEqual(intake.handler(event, None)["statusCode"], status)
        self.assertEqual(self.s3.objects, {})


class TranscribeTest(unittest.TestCase):
    def test_scrub_removes_private_details_and_defuses_mentions(self):
        text = transcribe.scrub(
            "See /Users/alice/proj/a.blend and C:\\Users\\bob\\x, mail a@b.com, host 10.1.2.3, "
            "id 5118889d-1b86-4ef8-8255-76cf59584d6d, key ghp_abcdefghijklmnopqrstuv, cc @octocat"
        )
        for secret in ("alice", "bob", "a@b.com", "10.1.2.3", "5118889d", "ghp_"):
            self.assertNotIn(secret, text)
        self.assertIn("`@octocat`", text)

    def test_decide_accepts_a_valid_answer_and_falls_back_safely(self):
        answer = json.dumps({"action": "duplicate", "duplicate_of": 7, "host": "blender",
                             "title": "GLB import  freezes", "body": "### Summary\nFrom /home/eve/x"})
        decision = transcribe.decide(answer, {7})
        self.assertEqual((decision["action"], decision["duplicate_of"], decision["host"]), ("duplicate", 7, "blender"))
        self.assertEqual(decision["title"], "GLB import freezes")
        self.assertNotIn("eve", decision["body"])

        unknown_issue = transcribe.decide(answer.replace('"blender"', '"photoshop"'), {8})
        self.assertEqual((unknown_issue["action"], unknown_issue["host"]), ("create", "core"))
        self.assertEqual(transcribe.decide('```json\n{"action": "spam"}\n```', set()), {"action": "spam"})

    def test_decide_rejects_unusable_answers(self):
        for answer in ("not json", "[]", '{"action": "delete"}', '{"action": "create", "title": "x"}'):
            with self.assertRaises(transcribe.Unusable):
                transcribe.decide(answer, set())


if __name__ == "__main__":
    unittest.main()
