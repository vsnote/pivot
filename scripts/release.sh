#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

BUMP_ARG=""
NO_PUSH=false
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-push)
      NO_PUSH=true
      ;;
    --dry-run)
      DRY_RUN=true
      ;;
    -*)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
    *)
      if [[ -n "${BUMP_ARG}" ]]; then
        echo "Unexpected extra version argument: $1" >&2
        exit 1
      fi
      BUMP_ARG="$1"
      ;;
  esac
  shift
done

BUMP_ARG="${BUMP_ARG:-patch}"

CURRENT_VER="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/pivot-desktop/Cargo.toml | head -n 1)"
if [[ -z "${CURRENT_VER}" ]]; then
  echo "Failed to read current version from crates/pivot-desktop/Cargo.toml" >&2
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "${CURRENT_VER%%-*}"

case "${BUMP_ARG}" in
  major)
    NEW_VER="$((MAJOR + 1)).0.0"
    ;;
  minor)
    NEW_VER="${MAJOR}.$((MINOR + 1)).0"
    ;;
  patch)
    NEW_VER="${MAJOR}.${MINOR}.$((PATCH + 1))"
    ;;
  *)
    NEW_VER="${BUMP_ARG#v}"
    ;;
esac

TAG_NAME="v${NEW_VER}"
echo "Preparing release: ${CURRENT_VER} -> ${NEW_VER} (${TAG_NAME})"

if [[ "${DRY_RUN}" == "true" ]]; then
  echo "[dry-run] Would update crates/pivot-core/Cargo.toml, crates/pivot-desktop/Cargo.toml, Cargo.lock and create tag ${TAG_NAME}"
  exit 0
fi

perl -i -pe "s/^version = \".*?\"/version = \"${NEW_VER}\"/" crates/pivot-core/Cargo.toml
perl -i -pe "s/^version = \".*?\"/version = \"${NEW_VER}\"/" crates/pivot-desktop/Cargo.toml

cargo check --workspace --quiet

git add crates/pivot-core/Cargo.toml crates/pivot-desktop/Cargo.toml Cargo.lock
git commit -m "chore(release): ${TAG_NAME}"
git tag -a "${TAG_NAME}" -m "Release ${TAG_NAME}"

echo "Created commit and tag: ${TAG_NAME}"

if [[ "${NO_PUSH}" == "false" ]]; then
  echo "Pushing HEAD and ${TAG_NAME} to origin..."
  git push origin HEAD
  git push origin "${TAG_NAME}"
  echo "Push succeeded. GitHub Actions release workflow has been triggered."
else
  echo "Tag created locally (--no-push). Push later via: git push origin HEAD && git push origin ${TAG_NAME}"
fi
