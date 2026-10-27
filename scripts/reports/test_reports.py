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
            ({**post(b"{}"), "body": "not base64!", "isBase64Encoded": True}, 400),
            (post(b"[" * 100_000), 400),
        ]
        for event, status in cases:
            self.assertEqual(intake.handler(event, None)["statusCode"], status)
        self.assertEqual(self.s3.objects, {})


class TranscribeTest(unittest.TestCase):
    def test_scrub_removes_private_details_and_defuses_mentions(self):
        text = transcribe.scrub(
            "See /Users/alice/proj/a.blend and C:\\Users\\bob\\x, mail a@b.com, host 10.1.2.3, "
            "id 5118889d-1b86-4ef8-8255-76cf59584d6d, key ghp_abcdefghijklmnopqrstuv, cc @octocat, "
            "msy_abcdefghijklmnopqrstuv, \\\\fileserver\\carol, fe80::1ff:fe23:4567:890a, "
            "x `foo @octo-org bar` y `@meshy-dev/core"
        )
        for secret in ("alice", "bob", "a@b.com", "10.1.2.3", "5118889d", "ghp_", "msy_", "fileserver", "fe80"):
            self.assertNotIn(secret, text)
        self.assertNotRegex(text, r"@[A-Za-z0-9]")
        self.assertIn("@\u200boctocat", text)

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

    def test_posts_leave_pending_first_and_stop_at_the_daily_cap(self):
        calls = []
        stored = {"report_id": "r1", "report": report()}

        def fake_run(*argv, stdin=None):
            calls.append(argv)
            if argv[:2] == ("aws", "s3api"):
                return json.dumps(["pending/a.json", "pending/b.json"])
            if argv[:2] == ("gh", "api"):
                return json.dumps([{"body": transcribe.MARKER + "old -->"}] * (transcribe.DAILY_POST_CAP - 1))
            if argv[:3] == ("gh", "issue", "list"):
                return "[]"
            if argv[:3] == ("aws", "s3", "cp"):
                return json.dumps(stored)
            if argv[:3] == ("gh", "issue", "create"):
                return "https://github.com/o/r/issues/5\n"
            return ""

        answer = json.dumps({"action": "create", "host": "blender", "title": "GLB import freezes", "body": "x"})
        os.environ.update(REPORTS_BUCKET="reports", GITHUB_REPOSITORY="o/r")
        original = transcribe.run, transcribe.ask_model
        transcribe.run, transcribe.ask_model = fake_run, lambda *_: answer
        try:
            self.assertEqual(transcribe.main(), 0)
        finally:
            transcribe.run, transcribe.ask_model = original
        moves = [(call[4], call[5]) for call in calls if call[:3] == ("aws", "s3", "mv")]
        self.assertEqual(moves, [("s3://reports/pending/a.json", "s3://reports/sending/a.json"),
                                 ("s3://reports/sending/a.json", "s3://reports/filed/a.json")])
        names = [call[:3] for call in calls]
        self.assertLess(names.index(("aws", "s3", "mv")), names.index(("gh", "issue", "create")))
        self.assertEqual(sum(call[:3] == ("aws", "s3", "cp") for call in calls), 1)

    def test_a_response_that_is_not_a_chat_completion_stops_the_run(self):
        class Response:
            def __init__(self, body):
                self.body = body

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self):
                return self.body

        os.environ.update(LLM_BASE_URL="https://models.example", LLM_MODEL="m", LLM_API_KEY="k\n")
        original = transcribe.urllib.request.urlopen
        try:
            transcribe.urllib.request.urlopen = lambda *_, **__: Response(b"OK")
            with self.assertRaises(SystemExit):
                transcribe.ask_model({"report": report()}, [])
            transcribe.urllib.request.urlopen = lambda *_, **__: Response(
                b'{"choices": [{"message": {"content": null}}]}')
            with self.assertRaises(transcribe.Unusable):
                transcribe.ask_model({"report": report()}, [])
        finally:
            transcribe.urllib.request.urlopen = original


if __name__ == "__main__":
    unittest.main()
