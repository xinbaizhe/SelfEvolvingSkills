#!/usr/bin/env bash
#
# Fails when a Chinese string is hardcoded in frontend source instead of living
# in a locale catalog. This is the objective answer to "is the UI actually
# translated?", and it stops new code from quietly reintroducing Chinese.
#
# Comments and developer-only `console.*` calls are deliberately exempt: they
# are never rendered, and this project writes them in Chinese on purpose.
#
# A line can also opt out with an `i18n-exempt` marker in a trailing comment.
# That is for Chinese that reaches the user but is not UI chrome - persisted
# document content, for instance, which must not change with the interface
# language. Every use should say why on the same line.
#
# Usage: scripts/check-i18n.sh [src-dir]     (default: <repo>/frontend/src)

set -uo pipefail

# Resolved from this script's own location rather than the caller's cwd, so it
# behaves the same from the repo root, from frontend/, and in CI.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SRC_DIR="${1:-$SCRIPT_DIR/../frontend/src}"

if [ ! -d "$SRC_DIR" ]; then
  echo "::error::$SRC_DIR not found"
  exit 1
fi

# GNU grep's -P needs a UTF-8 locale and refuses otherwise ("-P supports only
# unibyte and UTF-8 locales"). C.UTF-8 is present on the CI runners and in Git
# Bash, where the ambient locale is not UTF-8.
export LC_ALL="${LC_ALL:-C.UTF-8}"

HAN='[\x{4e00}-\x{9fff}]'

# `grep -rn` output is `path:lineno:content`, so the content starts after the
# second colon - anchoring on `:<optional space>` is what keeps these filters
# from matching a marker that merely appears somewhere inside a string.
offenders="$(
  grep -rnP --include='*.vue' --include='*.ts' "$HAN" "$SRC_DIR" \
    | grep -v '/i18n/locales/' \
    | grep -vP ':[0-9]+:[[:space:]]*(//|\*|/\*)' \
    | grep -v '<!--' \
    | grep -vP ':[0-9]+:[[:space:]]*console\.' \
    | grep -v 'i18n-exempt' \
    || true
)"

if [ -n "$offenders" ]; then
  count="$(printf '%s\n' "$offenders" | wc -l)"
  printf '%s\n' "$offenders"
  echo
  echo "::error::$count line(s) with hardcoded Chinese outside the locale catalogs"
  echo "Move these into frontend/src/i18n/locales/{en,zh-CN}/ and render them with t()."
  exit 1
fi

echo "i18n check passed: no hardcoded Chinese in $SRC_DIR (outside locale catalogs)"
