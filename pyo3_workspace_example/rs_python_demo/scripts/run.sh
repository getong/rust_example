#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

project_root="$(pwd -P)"
python_src_dir="${project_root}/python_src"
venv_dir="${python_src_dir}/.venv"
venv_python="${venv_dir}/bin/python"

requires_python="$(awk -F '"' '/^[[:space:]]*requires-python[[:space:]]*=/ { print $2; exit }' "${python_src_dir}/pyproject.toml")"
python_version=""
python_requirement_kind=""

if [[ "${requires_python}" =~ ^==[[:space:]]*([0-9]+)\.([0-9]+)\.\*$ ]]; then
    python_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}"
    python_requirement_kind="prefix"
elif [[ "${requires_python}" =~ ^==[[:space:]]*([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    python_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
    python_requirement_kind="exact"
elif [[ "${requires_python}" =~ ^\>=([0-9]+)\.([0-9]+)(\.([0-9]+))?$ ]]; then
    python_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}${BASH_REMATCH[3]}"
    python_requirement_kind="minimum"
else
    echo "Missing supported 'requires-python' in ${python_src_dir}/pyproject.toml. Use '==<major>.<minor>.*', '==<major>.<minor>.<patch>', or '>=<major>.<minor>[.<patch>]'" >&2
    exit 1
fi

version_at_least() {
    local version="$1"
    local minimum="$2"
    local version_major version_minor version_patch
    local minimum_major minimum_minor minimum_patch

    IFS=. read -r version_major version_minor version_patch _ <<< "${version}"
    IFS=. read -r minimum_major minimum_minor minimum_patch _ <<< "${minimum}"

    version_patch="${version_patch:-0}"
    minimum_patch="${minimum_patch:-0}"

    if (( version_major != minimum_major )); then
        (( version_major > minimum_major ))
        return
    fi

    if (( version_minor != minimum_minor )); then
        (( version_minor > minimum_minor ))
        return
    fi

    (( version_patch >= minimum_patch ))
}

venv_matches_requirement() {
    case "${python_requirement_kind}" in
        prefix)
            [[ "${current_python_version}" == "${python_version}."* ]]
            ;;
        exact)
            [ "${current_python_version}" = "${python_version}" ]
            ;;
        minimum)
            version_at_least "${current_python_version}" "${python_version}"
            ;;
        *)
            return 1
            ;;
    esac
}

read_venv_python_version() {
    if [ -f "${venv_dir}/pyvenv.cfg" ]; then
        awk -F ' = ' '$1 == "version_info" { print $2; exit }' "${venv_dir}/pyvenv.cfg"
    fi
}

current_python_version="$(read_venv_python_version)"
if ! venv_matches_requirement; then
    uv venv --python "${python_version}" --managed-python --clear "${venv_dir}"
    current_python_version="$(read_venv_python_version)"
fi

if [ -z "${current_python_version}" ]; then
    echo "Unable to determine Python version from ${venv_dir}/pyvenv.cfg" >&2
    exit 1
fi

uv sync --project "${python_src_dir}" --python "${venv_python}" --managed-python --locked

python_library_dir="$("${venv_python}" -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR") or "")')"

case "$(uname -s)" in
    Linux)
        if [ -n "${python_library_dir}" ] && [ -d "${python_library_dir}" ]; then
            export LD_LIBRARY_PATH="${python_library_dir}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
        fi
        ;;
esac

PYO3_ENVIRONMENT_SIGNATURE="cpython-${current_python_version}" PYO3_PYTHON="${venv_python}" cargo run
