# luaskills-debug

`luaskills-debug` is the standalone developer tool for [LuaSkills](https://github.com/LuaSkills/luaskills) skills.
It runs a skill through the same load and call path a host uses, checks a skill source against the packaging
rules, builds release packages, and creates new skills from a template.

It embeds the LuaSkills runtime as a library. Its major and minor version follow that runtime: 0.8.x embeds
`luaskills` 0.8.0, and only the patch number moves on its own.

## Install

Download `luaskills-debug-tool-<platform>.tar.gz` from [Releases](https://github.com/LuaSkills/luaskills-debug/releases)
and check it against `luaskills-debug-checksums.txt`. Platforms: `windows-x64`, `linux-x64`, `linux-arm64`,
`macos-x64`, `macos-arm64`. The Windows executable is signed with a Certum code-signing certificate; the Linux
executable ships with a detached OpenPGP signature, `bin/luaskills-debug.asc`.

The archive holds `bin/luaskills-debug`, launcher scripts (`debug.ps1`, `debug.sh`), `setup_runtime` scripts
that fetch Lua runtime packages into the bundled `runtime/` folder, `scripts/deps/fetch_managed_runtimes.*` for the
managed Python and Node runtimes a skill declares, and a `skills/` folder for one skill to debug.

## Commands

| Command | Purpose |
| --- | --- |
| `new <dir> [--layout standard\|platform] [--name <name>]` | Create a skill from the built-in template, with a release workflow |
| `validate --skill-path <dir>` | Check a skill source against the packaging rules; exits with an error on issues |
| `package --skill-path <dir> --out-dir <dir> [--layout standard\|platform]` | Build the release assets |
| `sync`, `inspect`, `list-tools`, `call` | Copy a skill into a runtime root and load or call it the way a host does |
| `probe-mirrors` | Measure GitHub mirror speed |

Run `luaskills-debug --help` for every option.

```bash
luaskills-debug new my-skill
luaskills-debug call --runtime-root .luaskills-debug --skill-path my-skill --tool hello --args-json "{\"name\":\"LuaSkills\"}"
luaskills-debug validate --skill-path my-skill
luaskills-debug package --skill-path my-skill --out-dir dist
```

### Release layouts

`package` writes one of two layouts:

- **standard** (default): `<id>-v<version>-skill.zip` for every system, plus `<id>-v<version>-checksums.txt`.
  Most skills use it.
- **platform**: one `<id>-v<version>-<platform>-skill.zip` per system that has bundled payloads in
  `dependencies.yaml`, the all-systems package (omit it with `--no-universal`), the package index
  `<id>-v<version>-packages.json`, and the checksums file. LuaSkills installs the package for the current system.

Packaging rules come from the embedded runtime (`luaskills::package`), so a package built here is exactly what that
runtime installs. Packages ship `skill.yaml`, `dependencies.yaml`, README and license files, and the `runtime`,
`node`, `python`, `native`, `help`, `overflow_templates`, `resources`, `licenses` and `schemas` directories; pass
`--include <name>` for other top-level entries. `package` warns when a build exceeds the LuaSkills Hub limits:
100 MiB per package, 600 MiB per version, 1 GiB expanded.

### Calling a skill

`call` copies the skill into `<runtime-root>/skills/<skill_id>` and runs the normal `load_from_roots -> call_skill`
path, so dependency roots, state, databases and context match real host execution.

- Every `call` injects the context a host provides for one developer invocation: `request_id`, `client_name` and
  `matched_client_pattern` are `luaskills-debug`, and `client_budget` carries `tool_result` and `file_read` scopes
  of 200000 bytes with unlimited lines. Pass `--context-file <path>`, or `--client-budget <json>` and
  `--tool-config <json>`, to reproduce one specific host profile.
- Sync skips build, checkout and debug-wrapper directories at every depth (`target`, `.git`, `.svn`, `node_modules`,
  `__pycache__`, `.luaskills-debug`). Packaging output and names the runtime root owns (`dist`, `out`, `temp`,
  `state`, `databases`, `dependencies`, `lua_packages`, `libs`) are skipped only directly under the package root.
- Skills with `sqlite.enable: true` get the tool's local SQLite host; see [the SQLite host notes](docs/zh-CN/sqlite-host.md).

## GitHub Action

Skill repositories build releases with this repository's composite action:

```yaml
- uses: actions/checkout@v4
- id: package
  uses: LuaSkills/luaskills-debug@v0.8.0
  with:
    layout: standard        # or platform
    expect-tag: ${{ github.ref_name }}
- run: gh release create "$GITHUB_REF_NAME" --generate-notes "${{ steps.package.outputs.out-dir }}"/*
  env:
    GH_TOKEN: ${{ github.token }}
```

Inputs: `skill-path`, `layout`, `include-universal`, `out-dir`, `skill-id`, `include` (one entry per line),
`expect-tag`, `version`. Outputs: `out-dir`, `report`. The action downloads the release that matches its own tag
and verifies it against `luaskills-debug-checksums.txt`. The workflow that `new` writes uses it.

## Develop

The tool depends on the published `luaskills` crate. To build against a local core checkout, copy
`.cargo/config.example.toml` to `.cargo/config.toml`; it patches `luaskills` to `../vulcan-luaskills`. Do not
commit `Cargo.lock` while the patch is active.

```bash
cargo test
cargo build --release
bash scripts/build/package_debug_tool.sh linux-x64 v0.8.0 target/release-packages
```

The managed Python/Node runtime example and its smoke scripts are described in
[docs/managed-runtime-smoke.md](docs/managed-runtime-smoke.md).

Releases run from the **Release** workflow: it builds all five platforms, signs the Windows and Linux executables,
verifies each packaged archive, and creates a draft GitHub release. Signing needs the repository secrets
`CERTUM_TOTP_EMAIL`, `CERTUM_TOTP_SECRET`, `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE`.

## License

MIT
