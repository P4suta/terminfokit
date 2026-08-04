#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
#
# SPDX-License-Identifier: MIT OR Apache-2.0

set -eu

destination=${1:-target/oracle}
archive="$destination/ncurses-6.6.tar.gz"
source_file="$destination/ncurses-6.6/misc/terminfo.src"
archive_sha256='355b4cbbed880b0381a04c46617b7656e362585d52e9cf84a67e2009b749ff11'
source_sha256='75673b421c25032306f7cdf26df57978c86ed9cf3d3fb16a6479233775f4f961'

mkdir -p "$destination"

# Reuse an archive that is already present and intact. The differential test is
# the strongest claim this project makes, and it is a required check, so it must
# not depend on upstream serving a download on every run: invisible-island.net
# answers 403 to cloud runners often enough to fail pull requests for reasons
# unrelated to the change. The pinned digest is what makes reuse safe, and it is
# verified again below either way.
if [ -f "$archive" ] &&
  printf '%s  %s\n' "$archive_sha256" "$archive" | sha256sum --check --status -
then
  echo "Reusing the verified ncurses 6.6 archive at $archive"
else
  curl --fail --location --proto '=https' --tlsv1.2 \
    --retry 5 --retry-delay 5 --retry-connrefused \
    'https://invisible-island.net/archives/ncurses/ncurses-6.6.tar.gz' \
    --output "$archive"
fi

printf '%s  %s\n' "$archive_sha256" "$archive" | sha256sum --check -
tar -xzf "$archive" -C "$destination"
printf '%s  %s\n' "$source_sha256" "$source_file" | sha256sum --check -
