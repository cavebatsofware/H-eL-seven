#!/usr/bin/env bash
#
# Build and publish a release image for hl7-ui: versions, tags, labels, and
# optionally pushes. Run hl7-defs-etl to populate defs/ first; the snapshots
# are bundled into the image.
#
# Usage:
#   scripts/release.sh              # build and tag only
#   scripts/release.sh --push       # build, tag, and push to the registry
#
# Configuration (environment):
#   IMAGE       registry/name        (default ghcr.io/cavebatsofware/hl7-ui)
#   VERSION     override version tag  (default: git describe, else crate version)
#   SOURCE_URL  OCI image source url  (default the GitHub repo)
#   PLATFORM    target platform       (default linux/amd64; arm64 needs buildx)
#
# Pushing requires a prior `docker login` to the registry (for ghcr.io, a
# personal access token with write:packages).

set -euo pipefail
cd "$(dirname "$0")/.."

IMAGE="${IMAGE:-ghcr.io/cavebatsofware/hl7-ui}"
SOURCE_URL="${SOURCE_URL:-https://github.com/cavebatsofware/H-eL-seven}"
PLATFORM="${PLATFORM:-linux/amd64}"

if ! ls defs/hl7-*.json >/dev/null 2>&1; then
  echo "error: no defs/hl7-*.json found. Generate them with hl7-defs-etl first." >&2
  exit 1
fi

# Version: an exact git tag if we are on one, else the crate version plus the
# short commit, with a -dirty suffix when the tree has uncommitted changes.
crate_version="$(sed -n 's/^version = "\(.*\)"/\1/p' hl7-ui/Cargo.toml | head -1)"
revision="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
dirty=""
git diff --quiet 2>/dev/null || dirty="-dirty"
if [ -z "${VERSION:-}" ]; then
  if tag="$(git describe --tags --exact-match 2>/dev/null)"; then
    VERSION="${tag}${dirty}"
  else
    VERSION="${crate_version}-g${revision}${dirty}"
  fi
fi
created="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "Building ${IMAGE}:${VERSION}  (platform ${PLATFORM}, revision ${revision})"

docker build \
  --platform "${PLATFORM}" \
  --label "org.opencontainers.image.version=${VERSION}" \
  --label "org.opencontainers.image.revision=${revision}" \
  --label "org.opencontainers.image.created=${created}" \
  --label "org.opencontainers.image.source=${SOURCE_URL}" \
  --tag "${IMAGE}:${VERSION}" \
  .

# Move :latest only for a clean, tagged release, so it never points at a
# work-in-progress build.
tag_latest=0
if [ -z "${dirty}" ] && git describe --tags --exact-match >/dev/null 2>&1; then
  docker tag "${IMAGE}:${VERSION}" "${IMAGE}:latest"
  tag_latest=1
fi

if [ "${1:-}" = "--push" ] || [ "${PUSH:-}" = "1" ]; then
  docker push "${IMAGE}:${VERSION}"
  [ "${tag_latest}" = "1" ] && docker push "${IMAGE}:latest"
  echo "Pushed ${IMAGE}:${VERSION}"
else
  echo "Built ${IMAGE}:${VERSION} (not pushed; re-run with --push)."
fi

echo "Pull with: docker pull ${IMAGE}:${VERSION}"
