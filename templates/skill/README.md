# {{SKILL_NAME}}

`{{SKILL_ID}}` is a [LuaSkills](https://github.com/LuaSkills/luaskills) skill.

## Develop

```bash
luaskills-debug validate --skill-path .
luaskills-debug call --runtime-root .luaskills-debug --skill-path . --tool hello --args-file examples/hello.args.json
```

## Release

Push a tag that matches `version` in `skill.yaml`, for example `v0.1.0`. The release workflow validates the
skill, builds its packages with `luaskills-debug package` and publishes them to the GitHub release. The
LuaSkills Hub syncs releases from this repository.

Packaging layout: `{{LAYOUT}}`. Set the repository variable `LUASKILLS_PACKAGE_LAYOUT` to `standard` or
`platform` to change it.
