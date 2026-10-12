#!/usr/bin/env bash
# Create or update what .github/workflows/release.yml needs in AWS
# (account 133081291590, us-west-2): the role meshy-dcc-github-deploy, which
# only the main branch of meshy-dev/meshy-dcc-core can assume through GitHub
# OIDC, and the npm token in Secrets Manager.
#
#   AWS_PROFILE=SRE-133081291590 scripts/release/aws/setup.sh
#   AWS_PROFILE=SRE-133081291590 scripts/release/aws/setup.sh --npm-token < token-file
#
# Safe to rerun. The token is read from stdin and never passed as an argument.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export AWS_REGION=us-west-2
role=meshy-dcc-github-deploy
secret=meshy-dcc/release/npm-token

if aws iam get-role --role-name "$role" > /dev/null 2>&1; then
    aws iam update-assume-role-policy --role-name "$role" --policy-document "file://$here/trust-policy.json"
else
    aws iam create-role --role-name "$role" \
        --description "meshy-dcc-core release workflow (GitHub OIDC, main only)" \
        --assume-role-policy-document "file://$here/trust-policy.json" > /dev/null
fi
aws iam put-role-policy --role-name "$role" --policy-name meshy-dcc-release \
    --policy-document "file://$here/permissions-policy.json"
echo "role $role is ready"

if [ "${1:-}" = "--npm-token" ]; then
    if aws secretsmanager describe-secret --secret-id "$secret" > /dev/null 2>&1; then
        aws secretsmanager put-secret-value --secret-id "$secret" --secret-string file:///dev/stdin > /dev/null
    else
        aws secretsmanager create-secret --name "$secret" \
            --description "npm automation token for meshy-dcc and @meshy-ai/meshy-dcc-*" \
            --secret-string file:///dev/stdin > /dev/null
    fi
    echo "secret $secret is set"
fi
