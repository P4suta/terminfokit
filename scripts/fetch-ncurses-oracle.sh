#!/usr/bin/env sh
set -eu

destination=${1:-target/oracle}
archive="$destination/ncurses-6.6.tar.gz"
source_file="$destination/ncurses-6.6/misc/terminfo.src"

mkdir -p "$destination"
curl --fail --location --proto '=https' --tlsv1.2 \
  'https://invisible-island.net/archives/ncurses/ncurses-6.6.tar.gz' \
  --output "$archive"
printf '%s  %s\n' \
  '355b4cbbed880b0381a04c46617b7656e362585d52e9cf84a67e2009b749ff11' \
  "$archive" | sha256sum --check -
tar -xzf "$archive" -C "$destination"
printf '%s  %s\n' \
  '75673b421c25032306f7cdf26df57978c86ed9cf3d3fb16a6479233775f4f961' \
  "$source_file" | sha256sum --check -
