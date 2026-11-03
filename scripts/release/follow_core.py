"""Follow LuaSkills core patch releases without manual edits.
无需手工修改即可跟随 LuaSkills 核心补丁版本。

The debugger's major and minor version follow the embedded `luaskills` crate; its patch number moves on
its own. `plan` finds the newest non-yanked `luaskills` patch on crates.io in the embedded series and, when
it is newer than the pinned one, names the next debugger patch. `apply` rewrites `Cargo.toml` for that plan.
A new core minor series is never followed automatically: it may change the API, so it needs a manual bump.
调试工具的主次版本跟随内嵌的 `luaskills` crate，补丁号独立递增。`plan` 在 crates.io 上查找内嵌系列中
最新且未撤回的 `luaskills` 补丁，比当前固定版本新时给出下一个调试工具补丁号；`apply` 按计划改写 `Cargo.toml`。
新的核心次版本系列绝不自动跟随：它可能改变 API，需要人工升级。
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request
from pathlib import Path

# The crates.io sparse index entry of the embedded crate: one JSON object per published version.
# 内嵌 crate 的 crates.io 稀疏索引条目：每个已发布版本一个 JSON 对象。
INDEX_URL = "https://index.crates.io/lu/as/luaskills"
PACKAGE_VERSION = re.compile(r'(?m)^version = "(?P<version>[^"]+)"(?=\r?$)')
CORE_REQUIREMENT = re.compile(r'(?m)^luaskills = "=(?P<version>[^"]+)"(?=\r?$)')


def parse_version(version: str) -> tuple[int, int, int]:
    """Parse one exact `x.y.z` version; anything else raises ValueError.
    解析一个精确 `x.y.z` 版本；其他形式抛出 ValueError。
    """
    match = re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version)
    if match is None:
        raise ValueError(f"Expected an exact x.y.z version, received {version!r}")
    return int(match[1]), int(match[2]), int(match[3])


def manifest_versions(manifest: str) -> tuple[str, str]:
    """Return the package version and the exact `luaskills` pin of `manifest`; both must be unique.
    返回 `manifest` 的包版本及精确 `luaskills` 固定版本；两者都必须唯一。
    """
    package = PACKAGE_VERSION.findall(manifest)
    core = CORE_REQUIREMENT.findall(manifest)
    if len(package) != 1 or len(core) != 1:
        raise ValueError("Cargo.toml must hold one package version and one exact luaskills pin")
    return package[0], core[0]


def published_versions(index: str) -> list[str]:
    """Return every non-yanked exact version listed by one sparse index entry.
    返回单个稀疏索引条目中全部未撤回的精确版本。
    """
    versions = []
    for line in index.splitlines():
        if line.strip():
            record = json.loads(line)
            if not record["yanked"] and re.fullmatch(r"\d+\.\d+\.\d+", record["vers"]):
                versions.append(record["vers"])
    return versions


def plan(manifest: str, published: list[str]) -> dict[str, str] | None:
    """Return `{core, version}` for the next follow release, or None when the pin is current.
    返回下一个跟随版本的 `{core, version}`；固定版本已是最新时返回 None。
    """
    package, core = manifest_versions(manifest)
    own = parse_version(package)
    pinned = parse_version(core)
    if own[:2] != pinned[:2]:
        raise ValueError(f"luaskills-debug {package} must share major.minor with luaskills {core}")
    series = [parse_version(item) for item in published if parse_version(item)[:2] == pinned[:2]]
    newest = max(series, default=pinned)
    if newest <= pinned:
        return None
    return {
        "core": ".".join(map(str, newest)),
        "version": f"{own[0]}.{own[1]}.{own[2] + 1}",
    }


def apply(manifest: str, core: str, version: str) -> str:
    """Return `manifest` with the package version and exact `luaskills` pin replaced.
    返回替换包版本及精确 `luaskills` 固定版本后的 `manifest`。
    """
    manifest_versions(manifest)
    manifest = PACKAGE_VERSION.sub(f'version = "{version}"', manifest, count=1)
    return CORE_REQUIREMENT.sub(f'luaskills = "={core}"', manifest, count=1)


def main() -> int:
    """Run `plan` or `apply` against the repository manifest.
    针对仓库清单执行 `plan` 或 `apply`。
    """
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    plan_command = commands.add_parser("plan")
    plan_command.add_argument("--github-output", type=Path)
    apply_command = commands.add_parser("apply")
    apply_command.add_argument("--core", required=True)
    apply_command.add_argument("--version", required=True)
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    args = parser.parse_args()
    manifest = args.manifest.read_text(encoding="utf-8")
    if args.command == "apply":
        args.manifest.write_text(apply(manifest, args.core, args.version), encoding="utf-8", newline="\n")
        return 0
    request = urllib.request.Request(INDEX_URL, headers={"User-Agent": "luaskills-debug-follow-core"})
    with urllib.request.urlopen(request, timeout=60) as response:
        result = plan(manifest, published_versions(response.read().decode("utf-8")))
    summary = "luaskills pin is current" if result is None else f"follow luaskills {result['core']} as {result['version']}"
    print(summary)
    if args.github_output is not None:
        with args.github_output.open("a", encoding="utf-8") as output:
            output.write(f"core={'' if result is None else result['core']}\n")
            output.write(f"version={'' if result is None else result['version']}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
