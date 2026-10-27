#!/usr/bin/env bash
# Create or update the bug-report intake in AWS (account 133081291590, us-west-2):
#
# - bucket meshy-dcc-reports-133081291590: private, encrypted, TLS only, and
#   every object is deleted 90 days after it reached its current folder;
# - Lambda meshy-dcc-report-intake (scripts/reports/intake.py) behind a public
#   function URL, at most 5 running at once, allowed only to write pending/,
#   with logs kept 90 days;
# - role meshy-dcc-github-reports, which only the main branch of
#   meshy-dev/meshy-dcc-core can assume through GitHub OIDC
#   (.github/workflows/reports.yml), to read and move reports and to read the
#   optional LLM key secret meshy-dcc/reports/llm-api-key.
#
#   AWS_PROFILE=SRE-133081291590 scripts/reports/aws/setup.sh
#   AWS_PROFILE=SRE-133081291590 scripts/reports/aws/setup.sh --llm-key < key-file
#
# Safe to rerun. Prints the function URL, which goes into REPORT_INTAKE_URL in
# crates/meshy-dcc. The key is read from stdin and never passed as an argument.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export AWS_REGION=us-west-2
account=133081291590
bucket=meshy-dcc-reports-$account
function=meshy-dcc-report-intake
intake_role=meshy-dcc-report-intake
github_role=meshy-dcc-github-reports
secret=meshy-dcc/reports/llm-api-key
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Bucket.
if ! aws s3api head-bucket --bucket "$bucket" 2> /dev/null; then
    aws s3api create-bucket --bucket "$bucket" \
        --create-bucket-configuration LocationConstraint="$AWS_REGION" > /dev/null
fi
aws s3api put-public-access-block --bucket "$bucket" --public-access-block-configuration \
    BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true
aws s3api put-bucket-encryption --bucket "$bucket" --server-side-encryption-configuration \
    '{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"}}]}'
aws s3api put-bucket-lifecycle-configuration --bucket "$bucket" --lifecycle-configuration \
    '{"Rules":[{"ID":"expire-reports","Status":"Enabled","Filter":{},"Expiration":{"Days":90}}]}' > /dev/null
cat > "$work/bucket-policy.json" << EOF
{"Version":"2012-10-17","Statement":[{"Sid":"TlsOnly","Effect":"Deny","Principal":"*","Action":"s3:*",
  "Resource":["arn:aws:s3:::$bucket","arn:aws:s3:::$bucket/*"],
  "Condition":{"Bool":{"aws:SecureTransport":"false"}}}]}
EOF
aws s3api put-bucket-policy --bucket "$bucket" --policy "file://$work/bucket-policy.json"
echo "bucket $bucket is ready"

# Intake function and its role.
cat > "$work/lambda-trust.json" << 'EOF'
{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}
EOF
cat > "$work/lambda-permissions.json" << EOF
{"Version":"2012-10-17","Statement":[
  {"Sid":"StorePending","Effect":"Allow","Action":"s3:PutObject","Resource":"arn:aws:s3:::$bucket/pending/*"},
  {"Sid":"Logs","Effect":"Allow","Action":["logs:CreateLogGroup","logs:CreateLogStream","logs:PutLogEvents"],
   "Resource":"arn:aws:logs:$AWS_REGION:$account:log-group:/aws/lambda/$function:*"}
]}
EOF
if aws iam get-role --role-name "$intake_role" > /dev/null 2>&1; then
    aws iam update-assume-role-policy --role-name "$intake_role" --policy-document "file://$work/lambda-trust.json"
else
    aws iam create-role --role-name "$intake_role" \
        --description "meshy-dcc report intake (Lambda)" \
        --assume-role-policy-document "file://$work/lambda-trust.json" > /dev/null
    sleep 10 # a new role takes a moment before Lambda can assume it
fi
aws iam put-role-policy --role-name "$intake_role" --policy-name meshy-dcc-report-intake \
    --policy-document "file://$work/lambda-permissions.json"

aws logs create-log-group --log-group-name "/aws/lambda/$function" 2> /dev/null || true
aws logs put-retention-policy --log-group-name "/aws/lambda/$function" --retention-in-days 90

(cd "$here/.." && zip -q "$work/intake.zip" intake.py)
if aws lambda get-function --function-name "$function" > /dev/null 2>&1; then
    aws lambda update-function-code --function-name "$function" \
        --zip-file "fileb://$work/intake.zip" > /dev/null
    aws lambda wait function-updated --function-name "$function"
    aws lambda update-function-configuration --function-name "$function" \
        --environment "Variables={REPORTS_BUCKET=$bucket}" --timeout 10 --memory-size 128 > /dev/null
else
    aws lambda create-function --function-name "$function" --runtime python3.12 \
        --handler intake.handler --role "arn:aws:iam::$account:role/$intake_role" \
        --zip-file "fileb://$work/intake.zip" --timeout 10 --memory-size 128 \
        --environment "Variables={REPORTS_BUCKET=$bucket}" > /dev/null
fi
aws lambda wait function-active-v2 --function-name "$function"
aws lambda put-function-concurrency --function-name "$function" --reserved-concurrent-executions 5 > /dev/null

if ! url=$(aws lambda get-function-url-config --function-name "$function" \
        --query FunctionUrl --output text 2> /dev/null); then
    url=$(aws lambda create-function-url-config --function-name "$function" \
        --auth-type NONE --query FunctionUrl --output text)
fi
# A public function URL needs both grants; adding one that exists fails harmlessly.
aws lambda add-permission --function-name "$function" --statement-id public-url \
    --action lambda:InvokeFunctionUrl --principal '*' --function-url-auth-type NONE > /dev/null 2>&1 || true
aws lambda add-permission --function-name "$function" --statement-id public-url-invoke \
    --action lambda:InvokeFunction --principal '*' --invoked-via-function-url > /dev/null 2>&1 || true
echo "function $function is ready"

# Role for the reports workflow.
cat > "$work/github-trust.json" << EOF
{"Version":"2012-10-17","Statement":[{"Effect":"Allow",
  "Principal":{"Federated":"arn:aws:iam::$account:oidc-provider/token.actions.githubusercontent.com"},
  "Action":"sts:AssumeRoleWithWebIdentity",
  "Condition":{"StringEquals":{"token.actions.githubusercontent.com:aud":"sts.amazonaws.com",
    "token.actions.githubusercontent.com:sub":[
      "repo:meshy-dev@160386200/meshy-dcc-core@1381158595:ref:refs/heads/main",
      "repo:meshy-dev/meshy-dcc-core:ref:refs/heads/main"]}}}]}
EOF
cat > "$work/github-permissions.json" << EOF
{"Version":"2012-10-17","Statement":[
  {"Sid":"ListReports","Effect":"Allow","Action":"s3:ListBucket","Resource":"arn:aws:s3:::$bucket",
   "Condition":{"StringLike":{"s3:prefix":"pending/*"}}},
  {"Sid":"MoveReports","Effect":"Allow","Action":["s3:GetObject","s3:PutObject","s3:DeleteObject"],
   "Resource":["arn:aws:s3:::$bucket/pending/*","arn:aws:s3:::$bucket/sending/*",
     "arn:aws:s3:::$bucket/filed/*","arn:aws:s3:::$bucket/rejected/*","arn:aws:s3:::$bucket/failed/*"]},
  {"Sid":"LlmKey","Effect":"Allow","Action":"secretsmanager:GetSecretValue",
   "Resource":"arn:aws:secretsmanager:$AWS_REGION:$account:secret:$secret-*"}
]}
EOF
if aws iam get-role --role-name "$github_role" > /dev/null 2>&1; then
    aws iam update-assume-role-policy --role-name "$github_role" --policy-document "file://$work/github-trust.json"
else
    aws iam create-role --role-name "$github_role" \
        --description "meshy-dcc-core reports workflow (GitHub OIDC, main only)" \
        --assume-role-policy-document "file://$work/github-trust.json" > /dev/null
fi
aws iam put-role-policy --role-name "$github_role" --policy-name meshy-dcc-reports \
    --policy-document "file://$work/github-permissions.json"
echo "role $github_role is ready"

if [ "${1:-}" = "--llm-key" ]; then
    if aws secretsmanager describe-secret --secret-id "$secret" > /dev/null 2>&1; then
        aws secretsmanager put-secret-value --secret-id "$secret" --secret-string file:///dev/stdin > /dev/null
    else
        aws secretsmanager create-secret --name "$secret" \
            --description "API key for the meshy-dcc reports transcription model" \
            --secret-string file:///dev/stdin > /dev/null
    fi
    echo "secret $secret is set"
fi

# An invalid report proves the URL is public without storing anything.
status=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'content-type: application/json' -d '{}' "$url")
if [ "$status" != 400 ]; then
    echo "warning: POST $url returned $status, expected 400 for an invalid report" >&2
fi
echo "intake URL: $url"
