#!/usr/bin/env bash
# Publish a new version of Abode.
#
#   scripts/release.sh 0.2.0
#
# Bumps the version, commits, tags v0.2.0 and pushes. GitHub Actions then
# builds the Mac and Windows installers and publishes the release (about 15
# minutes). Everyone's Abode offers the update next time they open it.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ $# -lt 1 ]; then
  echo "Usage: scripts/release.sh <version>   e.g. scripts/release.sh 0.2.0" >&2
  exit 1
fi
V="${1#v}"
if ! [[ "$V" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version should look like 1.2.3" >&2
  exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "You have uncommitted changes. Commit them first." >&2
  exit 1
fi
if git rev-parse "v$V" >/dev/null 2>&1; then
  echo "v$V already exists." >&2
  exit 1
fi

perl -0pi -e 's/(\[workspace\.package\]\nversion = ")[^"]*"/${1}'"$V"'"/' Cargo.toml
cargo check -q -p abode-app            # refreshes Cargo.lock
cargo test -q --workspace >/dev/null
git commit -q -am "Release v$V"
git tag -a "v$V" -m "Abode $V"
git push -q origin HEAD "v$V"

repo=$(git remote get-url origin | sed -E 's#.*github.com[:/](.*)\.git$#\1#; s#.*github.com[:/](.*)$#\1#')
echo "Released v$V. Installers are building at https://github.com/$repo/actions"
echo "The release will appear at https://github.com/$repo/releases/tag/v$V"
