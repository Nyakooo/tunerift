#!/usr/bin/env bash
# Withdraw explicitly superseded releases before publishing their replacement.
set -euo pipefail
: "${GH_REPO:?GH_REPO is required}"
: "${CURRENT_TAG:?CURRENT_TAG is required}"
while IFS= read -r tag || [[ -n "$tag" ]]; do
    [[ -z "$tag" || "$tag" == \#* ]] && continue
    [[ "$tag" =~ ^v[0-9][a-zA-Z0-9.+-]*$ ]] || { echo "Invalid retired tag: $tag" >&2; exit 1; }
    [[ "$tag" != "$CURRENT_TAG" ]] || { echo 'Cannot retire the current release' >&2; exit 1; }
    echo "Withdrawing superseded release: $tag"
    runs=$(gh api --method GET "repos/$GH_REPO/actions/runs" -f "branch=$tag" --paginate --jq '.workflow_runs[] | select(.status != "completed") | .id')
    for run in $runs; do
        if ! gh api --method POST "repos/$GH_REPO/actions/runs/$run/cancel"; then
            [[ $(gh api "repos/$GH_REPO/actions/runs/$run" --jq '.status') == completed ]] || exit 1
        fi
        for ((attempt=0; attempt<60; attempt++)); do
            status=$(gh api "repos/$GH_REPO/actions/runs/$run" --jq '.status')
            [[ "$status" == completed ]] && break
            sleep 2
        done
        [[ "$status" == completed ]] || { echo "Run $run has not stopped; refusing to race release publication" >&2; exit 1; }
    done
    releases=$(gh api "repos/$GH_REPO/releases" --paginate --jq ".[] | select(.tag_name == \"$tag\") | .id")
    for release in $releases; do
        gh api --method DELETE "repos/$GH_REPO/releases/$release"
    done
    ref=$(gh api "repos/$GH_REPO/git/matching-refs/tags/$tag" --jq ".[] | select(.ref == \"refs/tags/$tag\") | .ref")
    if [[ -n "$ref" ]]; then
        gh api --method DELETE "repos/$GH_REPO/git/refs/tags/$tag"
    fi
done < .github/retired-tags.txt
