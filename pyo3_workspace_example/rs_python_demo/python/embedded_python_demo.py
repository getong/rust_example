import os
import sys

from packaging import version


def build_message() -> str:
    user = os.getenv("USER") or os.getenv("USERNAME") or "Unknown"
    parsed_version = str(version.Version("1.2.3"))
    return (
        f"Hello {user}, Python version {sys.version}, "
        f"packaging parsed {parsed_version}"
    )


def main() -> str:
    return build_message()
