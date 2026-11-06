#!/bin/bash
# Uploads rust/dist/OpenCut.app, built by rust/scripts/release-mac.sh, as the GitHub
# pre-release `latest`. The release and its tag are recreated on the current commit,
# so the source archives match the build.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cd -- "$repo_root"

app="rust/dist/OpenCut.app"
codesign --verify --deep --strict "$app"

commit="$(git rev-parse HEAD)"
git fetch --quiet origin
if [[ -z "$(git branch --remotes --contains "$commit")" ]]; then
    echo "The current commit $commit is not pushed to origin" >&2
    exit 1
fi

zip="$(mktemp -d)/OpenCut-macOS.zip"
ditto -c -k --keepParent "$app" "$zip"

if gh release view latest >/dev/null 2>&1; then
    gh release delete latest --cleanup-tag --yes
fi
gh release create latest "$zip" \
    --target "$commit" \
    --title "Latest" \
    --prerelease \
    --notes "macOS build of OpenCut from $(git rev-parse --short HEAD).

The app is not notarized. After unzipping, run:

    xattr -cr OpenCut.app

then open it."
