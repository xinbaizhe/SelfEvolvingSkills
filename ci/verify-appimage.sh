#!/usr/bin/env bash
# Verifies a built AppImage the way AppDir linters and the AppImage catalog's
# test harness do, plus a glibc symbol-version ceiling check.
#
# Usage: verify-appimage.sh <path-to-AppImage> [max-glibc]   (default 2.35)
#
# Exits non-zero and emits ::error:: annotations on any failure.

set -euo pipefail

APPIMAGE="${1:?usage: verify-appimage.sh <path-to-AppImage> [max-glibc]}"
MAX_GLIBC="${2:-2.35}"

[ -f "$APPIMAGE" ] || { echo "::error::not a file: $APPIMAGE"; exit 1; }
APPIMAGE="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"
echo "Verifying: $APPIMAGE"

chmod +x "$APPIMAGE"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"
"$APPIMAGE" --appimage-extract >/dev/null

fail=0

# `test -e` follows symlinks, so a dangling .DirIcon fails here too.
if [ ! -e squashfs-root/.DirIcon ]; then
  echo "::error::.DirIcon is missing or dangling in the AppImage"
  fail=1
else
  echo ".DirIcon -> $(readlink squashfs-root/.DirIcon || echo '(regular file)')"
fi

# Symlinks at the AppDir root must be relative (resolve inside the AppDir).
for link in squashfs-root/.DirIcon squashfs-root/*.desktop; do
  [ -L "$link" ] || continue
  target=$(readlink "$link")
  case "$target" in
    /*) echo "::error::$(basename "$link") is an absolute symlink -> $target"; fail=1 ;;
  esac
done

ls squashfs-root/*.desktop >/dev/null 2>&1 || {
  echo "::error::no .desktop file at the AppDir root"
  fail=1
}

echo "AppDir OK"

# The Linux build runs in an ubuntu:22.04 container (glibc 2.35) so the artifact
# stays runnable on Ubuntu 22.04 / Debian 12 and on the AppImage catalog's
# test runner. A GLIBC_2.38+ symbol here means the build image was moved back to
# something newer - catch it here rather than in a downstream user's crash log.
echo "Checking glibc symbol ceiling (allowed: <= GLIBC_$MAX_GLIBC)"
observed_max=""
offenders=0
while IFS= read -r elf; do
  worst=$(objdump -T "$elf" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' | sort -uV | tail -1 || true)
  [ -n "$worst" ] || continue
  if [ -z "$observed_max" ] || [ "$(printf '%s\n' "$worst" "$observed_max" | sort -V | tail -1)" = "$worst" ]; then
    observed_max="$worst"
  fi
  if [ "$(printf '%s\n' "$worst" "GLIBC_$MAX_GLIBC" | sort -V | tail -1)" != "GLIBC_$MAX_GLIBC" ]; then
    offenders=$((offenders + 1))
    # A build image that drifted a release or two newer trips this on dozens of
    # bundled libraries at once. Report the first few in full and count the rest,
    # so the log stays readable.
    if [ "$offenders" -le 10 ]; then
      echo "::error::$(basename "$elf") requires $worst (ceiling: GLIBC_$MAX_GLIBC)"
    fi
    fail=1
  fi
done < <(find squashfs-root/usr/bin squashfs-root/usr/lib -maxdepth 2 -type f 2>/dev/null)
if [ "$offenders" -gt 10 ]; then
  echo "::error::... and $((offenders - 10)) more files exceed the glibc ceiling"
fi
echo "Highest glibc symbol version required: ${observed_max:-none found} (ceiling GLIBC_$MAX_GLIBC)"

[ "$fail" -eq 0 ] || exit 1
echo "AppImage OK"
