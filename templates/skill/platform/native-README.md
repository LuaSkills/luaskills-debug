# Native libraries

Put one compiled library per system here, at the paths that `dependencies.yaml` declares:

| Platform | File |
| --- | --- |
| windows-x64 | `native/windows-x64/{{LIB_STEM}}.dll` |
| linux-x64 | `native/linux-x64/lib{{LIB_STEM}}.so` |
| linux-arm64 | `native/linux-arm64/lib{{LIB_STEM}}.so` |
| macos-x64 | `native/macos-x64/lib{{LIB_STEM}}.dylib` |
| macos-arm64 | `native/macos-arm64/lib{{LIB_STEM}}.dylib` |

Remove the systems you do not support from `dependencies.yaml`. A release with the platform layout
publishes one package per listed system, plus `{{SKILL_ID}}-v<version>-packages.json`, which tells LuaSkills
which package to install. A system without its own package receives the all-systems package, unless the
release disables it with `include-universal: false`.

Check the layout locally:

```bash
luaskills-debug validate --skill-path .
luaskills-debug package --skill-path . --out-dir dist --layout platform
```
