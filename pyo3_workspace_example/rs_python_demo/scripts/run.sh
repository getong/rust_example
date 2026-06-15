#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

project_root="$(pwd -P)"
venv_dir="${project_root}/.venv"
venv_python="${venv_dir}/bin/python"

requires_python="$(awk -F '"' '/^[[:space:]]*requires-python[[:space:]]*=/ { print $2; exit }' pyproject.toml)"
python_version=""
python_version_is_prefix=false

if [[ "${requires_python}" =~ ^==[[:space:]]*([0-9]+)\.([0-9]+)\.\*$ ]]; then
    python_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}"
    python_version_is_prefix=true
elif [[ "${requires_python}" =~ ^==[[:space:]]*([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    python_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
else
    echo "Missing exact 'requires-python = \"==<major>.<minor>.*\"' or 'requires-python = \"==<major>.<minor>.<patch>\"' in pyproject.toml" >&2
    exit 1
fi

read_venv_python_version() {
    if [ -f "${venv_dir}/pyvenv.cfg" ]; then
        awk -F ' = ' '$1 == "version_info" { print $2; exit }' "${venv_dir}/pyvenv.cfg"
    fi
}

current_python_version="$(read_venv_python_version)"
if {
    [ "${python_version_is_prefix}" = true ] && [[ "${current_python_version}" != "${python_version}."* ]];
} || {
    [ "${python_version_is_prefix}" = false ] && [ "${current_python_version}" != "${python_version}" ];
}; then
    uv venv --python "${python_version}" --managed-python --clear "${venv_dir}"
    current_python_version="$(read_venv_python_version)"
fi

if [ -z "${current_python_version}" ]; then
    echo "Unable to determine Python version from ${venv_dir}/pyvenv.cfg" >&2
    exit 1
fi

uv pip install --python "${venv_python}" -r requirement.txt

python_library_dir="$("${venv_python}" -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR") or "")')"

case "$(uname -s)" in
    Linux)
        if [ -n "${python_library_dir}" ] && [ -d "${python_library_dir}" ]; then
            export LD_LIBRARY_PATH="${python_library_dir}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
        fi
        ;;
esac

PYO3_ENVIRONMENT_SIGNATURE="cpython-${current_python_version}" PYO3_PYTHON="${venv_python}" cargo run
