#!/usr/bin/env bash
# Publish a new version of Abode.
#
#   scripts/release.sh 0.2.0
#
# main only takes changes through pull requests, so this:
#   1. bumps the version on a release-v0.2.0 branch made from the latest main
#      (in a temporary worktree, so your own checkout is left alone),
#   2. pushes it and opens a pull request,
#   3. waits for the CI checks, then merges the pull request,
#   4. tags the merged commit v0.2.0 and pushes the tag.
# GitHub Actions then builds the Mac and Windows installers and publishes the
# release (about 15 minutes). Everyone's Abode offers the update next time
# they open it.
#
# If it stops part way (failed checks, no network...), fix the problem and run
# the same command again: it picks up the existing pull request.
#
# Needs the GitHub CLI (`gh`), signed in.
set -euo pipefail

main() {
  cd "$(dirname "$0")/.."
  local root
  root=$(pwd)

  if [ $# -lt 1 ]; then
    echo "Usage: scripts/release.sh <version>   e.g. scripts/release.sh 0.2.0" >&2
    exit 1
  fi
  local V="${1#v}"
  if ! [[ "$V" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version should look like 1.2.3" >&2
    exit 1
  fi
  if ! command -v gh >/dev/null; then
    echo "This needs the GitHub CLI: brew install gh, then gh auth login" >&2
    exit 1
  fi
  gh auth status >/dev/null 2>&1 || { echo "Sign in to GitHub first: gh auth login" >&2; exit 1; }

  local branch="release-v$V"
  local repo
  repo=$(git remote get-url origin | sed -E 's#.*github.com[:/](.*)\.git$#\1#; s#.*github.com[:/](.*)$#\1#')

  git fetch -q origin main --tags
  if git rev-parse -q --verify "refs/tags/v$V" >/dev/null; then
    echo "v$V already exists." >&2
    exit 1
  fi

  # Pick up a pull request from an earlier run, or make one.
  local pr state
  pr=$(gh pr list --head "$branch" --state all --json number --jq '.[0].number // empty')
  if [ -n "$pr" ]; then
    state=$(gh pr view "$pr" --json state --jq .state)
    if [ "$state" = "CLOSED" ]; then
      echo "Pull request #$pr for $branch was closed without merging." >&2
      echo "Reopen it, or delete the $branch branch on GitHub and run this again." >&2
      exit 1
    fi
    echo "Continuing with pull request #$pr."
  else
    open_pr "$root" "$V" "$branch"
    pr=$(gh pr list --head "$branch" --state open --json number --jq '.[0].number')
    state=OPEN
  fi

  if [ "$state" = "OPEN" ]; then
    wait_for_checks "$pr"
    echo "Merging pull request #$pr..."
    gh pr merge "$pr" --merge --subject "Release v$V" >/dev/null
  fi

  # Tag the commit that landed on main and push the tag.
  local commit
  commit=$(gh pr view "$pr" --json mergeCommit --jq '.mergeCommit.oid')
  git fetch -q origin main
  git tag -a "v$V" "$commit" -m "Abode $V"
  git push -q origin "v$V"
  git push -q origin --delete "$branch" 2>/dev/null || true
  git branch -q -D "$branch" 2>/dev/null || true

  echo
  echo "Released v$V. Installers are building at https://github.com/$repo/actions"
  echo "The release will appear at https://github.com/$repo/releases/tag/v$V"
  echo "Run 'git pull' on main to pick up the version bump."
}

# Bump the version on a fresh branch from origin/main and open a pull request.
open_pr() {
  local root=$1 V=$2 branch=$3
  local wt
  wt=$(mktemp -d "${TMPDIR:-/tmp}/abode-release.XXXXXX")
  # Remove the worktree however this ends (including a failed check or test).
  trap "git -C '$root' worktree remove --force '$wt' >/dev/null 2>&1; git -C '$root' branch -q -D '$branch' 2>/dev/null; true" EXIT

  git worktree add -q -B "$branch" "$wt" origin/main
  (
    cd "$wt"
    local current
    current=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
    if [ "$current" = "$V" ]; then
      echo "main is already at version $V." >&2
      exit 1
    fi
    perl -0pi -e 's/(\[workspace\.package\]\nversion = ")[^"]*"/${1}'"$V"'"/' Cargo.toml
    # Share the build cache with the main checkout so this isn't a full rebuild.
    export CARGO_TARGET_DIR="$root/target"
    echo "Checking and testing $current -> $V..."
    cargo check -q -p abode-app            # refreshes Cargo.lock
    cargo test -q --workspace >/dev/null
    git commit -q -am "Release v$V"
    git push -q -u --force origin "$branch"   # replaces a half-finished earlier run
    gh pr create --base main --head "$branch" --title "Release v$V" \
      --body "Bumps the version to $V. Once merged, \`scripts/release.sh\` tags the merge commit \`v$V\`, which builds and publishes the installers." >/dev/null
  )
  git worktree remove --force "$wt"
  trap - EXIT
  echo "Opened a pull request for $branch."
}

# Wait for the pull request's CI checks to appear and pass.
wait_for_checks() {
  local pr=$1 tries=0
  echo "Waiting for CI on pull request #$pr (this takes a few minutes)..."
  until [ "$(gh pr checks "$pr" --json name --jq length 2>/dev/null || echo 0)" -gt 0 ]; do
    tries=$((tries + 1))
    if [ $tries -gt 40 ]; then
      echo "No CI checks started after 10 minutes. Check https://github.com/$(gh repo view --json nameWithOwner --jq .nameWithOwner)/pull/$pr" >&2
      exit 1
    fi
    sleep 15
  done
  if ! gh pr checks "$pr" --watch --fail-fast --interval 20 >/dev/null; then
    echo "CI failed on pull request #$pr:" >&2
    gh pr checks "$pr" >&2 || true
    echo "Fix it (push to the $(gh pr view "$pr" --json headRefName --jq .headRefName) branch), then run this again." >&2
    exit 1
  fi
  echo "CI passed."
}

main "$@"
