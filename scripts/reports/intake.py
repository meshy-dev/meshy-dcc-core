"""Bug-report intake for `meshy-dcc report`: an AWS Lambda behind a public function URL.

It accepts one Report v1 JSON body, checks its shape and size, and stores it as
``s3://$REPORTS_BUCKET/pending/<report id>.json``. The reports workflow
(.github/workflows/reports.yml) picks it up from there. That is all it does: no
GitHub credentials live here, and a stored report is never served back.

Abuse is bounded by the body cap below, the function's reserved concurrency and
the bucket's 90-day expiry (scripts/reports/aws/setup.sh), and by the daily issue
cap in the workflow.
"""

from __future__ import annotations

import base64
import json
import os
import secrets
import time

SCHEMA = "meshy-dcc.report.v1"
MAX_BYTES = 256 * 1024
MAX_SUMMARY = 4000
MAX_FINDINGS = 20
SOURCES = ("cli", "ui")

_s3 = None


def handler(event: dict, _context: object) -> dict:
    method = event.get("requestContext", {}).get("http", {}).get("method", "")
    if method != "POST":
        return reply(405, {"error": "method not allowed"})
    body = event.get("body") or ""
    try:
        raw = base64.b64decode(body, validate=True) if event.get("isBase64Encoded") else body.encode()
        if len(raw) > MAX_BYTES:
            return reply(413, {"error": "report too large"})
        report = json.loads(raw)
    except (ValueError, RecursionError):
        return reply(400, {"error": "invalid report: not JSON"})
    problem = check(report)
    if problem:
        return reply(400, {"error": f"invalid report: {problem}"})

    report_id = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + secrets.token_hex(4)
    stored = {"report_id": report_id, "received_at": int(time.time()), "report": report}
    s3().put_object(
        Bucket=os.environ["REPORTS_BUCKET"],
        Key=f"pending/{report_id}.json",
        Body=json.dumps(stored).encode(),
        ContentType="application/json",
    )
    return reply(202, {"report_id": report_id})


def check(report: object) -> str | None:
    """Return why ``report`` is not a Report v1, or None."""
    if not isinstance(report, dict):
        return "not an object"
    if report.get("schema") != SCHEMA:
        return "unknown schema"
    if report.get("source") not in SOURCES:
        return "unknown source"
    summary = report.get("summary")
    if not isinstance(summary, str) or not summary.strip() or len(summary) > MAX_SUMMARY:
        return f"summary must be 1 to {MAX_SUMMARY} characters"
    client = report.get("client")
    if not isinstance(client, dict) or not all(
        isinstance(client.get(key), str) and 0 < len(client[key]) <= 64
        for key in ("version", "os", "arch")
    ):
        return "client must name version, os and arch"
    if not isinstance(report.get("environment", {}), dict):
        return "environment must be an object"
    findings = report.get("findings", [])
    if not isinstance(findings, list) or len(findings) > MAX_FINDINGS:
        return f"findings must be a list of at most {MAX_FINDINGS}"
    if not all(isinstance(finding, dict) for finding in findings):
        return "every finding must be an object"
    return None


def reply(status: int, payload: dict) -> dict:
    return {
        "statusCode": status,
        "headers": {"content-type": "application/json"},
        "body": json.dumps(payload),
    }


def s3():
    # boto3 ships with the Lambda runtime; imported here so the tests run without it.
    global _s3
    if _s3 is None:
        import boto3

        _s3 = boto3.client("s3")
    return _s3
