#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

generate_index() {
  local rel_dir="$1"
  local pattern="$2"
  local dir="${ROOT_DIR}/${rel_dir}"
  local out="${dir}/index.json"

  mkdir -p "${dir}"

  mapfile -t files < <(
    find "${dir}" -maxdepth 1 -type f -name "${pattern}" ! -name "index.json" -printf "%f\n" | sort
  )

  {
    printf '{\n'
    printf '  "files": ['
    if [[ ${#files[@]} -eq 0 ]]; then
      printf ']\n'
    else
      printf '\n'
      for i in "${!files[@]}"; do
        local comma=","
        if [[ "${i}" -eq $((${#files[@]} - 1)) ]]; then
          comma=""
        fi
        printf '    "%s"%s\n' "${files[$i]}" "${comma}"
      done
      printf '  ]\n'
    fi
    printf '}\n'
  } > "${out}"
}

generate_index "src/structure" "*.json"
generate_index "src/entity/behaviour" "*.yaml"
generate_index "src/entity/trait" "*.yaml"
generate_index "src/entity/enemy" "*.yaml"
generate_index "src/entity/friend" "*.yaml"
generate_index "src/entity/misc" "*.yaml"
generate_index "src/particle" "*.yaml"

printf 'WASM index manifests generated.\n'
