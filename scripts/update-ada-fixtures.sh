#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_revision="${1:-b2c2d7f6b5723a4b924409f9d80ce517d6db8226}"
ada_checkout="${repository_root}/target/ada-fixtures"
fixture_destination="${repository_root}/tests/fixtures/ada"

if [[ ! -d "${ada_checkout}/.git" ]]; then
  git clone https://github.com/ada-url/ada.git "${ada_checkout}"
fi
git -C "${ada_checkout}" fetch origin "${ada_revision}"
git -C "${ada_checkout}" checkout --detach "${ada_revision}"

fixtures=(
  IdnaTestV2.json
  percent-encoding.json
  setters_tests.json
  ada_extra_setters_tests.json
  toascii.json
  urltestdata.json
  ada_extra_urltestdata.json
  verifydnslength_tests.json
)

mkdir -p "${fixture_destination}/wpt"
for fixture in "${fixtures[@]}"; do
  cp "${ada_checkout}/tests/wpt/${fixture}" "${fixture_destination}/wpt/${fixture}"
done
cp "${ada_checkout}/LICENSE-APACHE" "${fixture_destination}/LICENSE-APACHE"
cp "${ada_checkout}/LICENSE-MIT" "${fixture_destination}/LICENSE-MIT"
printf '%s\n' "${ada_revision}" >"${fixture_destination}/REVISION"

echo "Updated Ada compatibility fixtures to ${ada_revision}"
