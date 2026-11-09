#!/usr/bin/env bash
# Create or update what .github/workflows/release.yml needs in AWS
# (account 133081291590, us-west-2): the role meshy-dcc-github-deploy, which
# only the main branches of meshy-dev/meshy-dcc-core and
# meshy-dev/meshy-dcc-adapters (whose release workflow copies adapter builds to
# the CDN) can assume through GitHub OIDC. npm needs nothing here: the workflow
# publishes through npm trusted publishing.
# The repositories use GitHub's immutable OIDC subjects, which carry the owner
# and repository IDs (repo:meshy-dev@<id>/<repo>@<id>:ref:...); see
# `gh api repos/meshy-dev/<repo>/actions/oidc/customization/sub`.
#
#   AWS_PROFILE=SRE-133081291590 scripts/release/aws/setup.sh
#
# Safe to rerun.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export AWS_REGION=us-west-2
role=meshy-dcc-github-deploy

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

