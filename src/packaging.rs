//! Skill source validation, release packaging and new-skill scaffolding.
//! 技能源校验、发布打包与新技能脚手架。
//!
//! Packaging rules live in the `luaskills::package` module of the runtime, so a package built here is exactly
//! what that runtime installs. This module only parses command lines and renders reports.
//! 打包规则位于运行时的 `luaskills::package` 模块，因此这里构建的包与该运行时安装的包完全一致。
//! 本模块只解析命令行并渲染报告。

use super::{DebugOutputMode, read_cli_value};
use luaskills::lua_skill::validate_luaskills_identifier;
use luaskills::package::{
    PackageAssetKind, PackageBuildReport, PackageBuildRequest, PackageLayout, SkillSourceOptions,
    SkillValidationReport, build_skill_release, validate_skill_source,
};
use luaskills::runtime::render_host_visible_path;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Largest single package the LuaSkills Hub accepts.
/// LuaSkills Hub 接受的单个包上限。
const HUB_PACKAGE_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

/// Largest combined package size of one version the LuaSkills Hub accepts.
/// LuaSkills Hub 接受的单个版本全部包合计上限。
const HUB_VERSION_LIMIT_BYTES: u64 = 600 * 1024 * 1024;

/// Largest expanded skill size the LuaSkills Hub accepts.
/// LuaSkills Hub 接受的技能解压后上限。
const HUB_EXPANDED_LIMIT_BYTES: u64 = 1024 * 1024 * 1024;

/// One file of the `new` skill template with its embedded text.
/// `new` 技能模板中的单个文件及其内嵌文本。
struct TemplateFile {
    /// Path relative to the new skill directory.
    /// 相对新技能目录的路径。
    path: &'static str,
    /// Template text with `{{...}}` placeholders.
    /// 带 `{{...}}` 占位符的模板文本。
    text: &'static str,
}

/// Files every new skill receives.
/// 每个新技能都会获得的文件。
const COMMON_TEMPLATE_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "skill.yaml",
        text: include_str!("../templates/skill/skill.yaml"),
    },
    TemplateFile {
        path: "runtime/hello.lua",
        text: include_str!("../templates/skill/runtime/hello.lua"),
    },
    TemplateFile {
        path: "help/help.md",
        text: include_str!("../templates/skill/help/help.md"),
    },
    TemplateFile {
        path: "help/hello.md",
        text: include_str!("../templates/skill/help/hello.md"),
    },
    TemplateFile {
        path: "examples/hello.args.json",
        text: include_str!("../templates/skill/examples/hello.args.json"),
    },
    TemplateFile {
        path: "README.md",
        text: include_str!("../templates/skill/README.md"),
    },
    TemplateFile {
        path: ".gitignore",
        text: include_str!("../templates/skill/gitignore.txt"),
    },
    TemplateFile {
        path: ".github/workflows/release.yml",
        text: include_str!("../templates/skill/release.yml"),
    },
];

/// Extra files of a new skill that uses the platform layout.
/// 使用平台布局的新技能额外获得的文件。
const PLATFORM_TEMPLATE_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "dependencies.yaml",
        text: include_str!("../templates/skill/platform/dependencies.yaml"),
    },
    TemplateFile {
        path: "native/README.md",
        text: include_str!("../templates/skill/platform/native-README.md"),
    },
];

/// Options shared by `validate` and `package` that select one skill source.
/// `validate` 与 `package` 共享的技能源选择选项。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceArgs {
    /// Skill source directory that contains skill.yaml.
    /// 包含 skill.yaml 的技能源目录。
    skill_path: Option<PathBuf>,
    /// Explicit skill id; the directory name when omitted.
    /// 显式技能标识；省略时使用目录名。
    skill_id: Option<String>,
    /// Extra top-level entries shipped beside the default ones.
    /// 在默认条目之外额外打包的顶层条目。
    include: Vec<String>,
}

impl SourceArgs {
    /// Try to consume one source option at `index`; return whether it was recognized.
    /// 尝试在 `index` 处消费一个源选项；返回是否识别。
    fn take(&mut self, args: &[String], index: &mut usize) -> Result<bool, String> {
        let flag = args[*index].as_str();
        match flag {
            "--skill-path" => {
                self.skill_path = Some(PathBuf::from(read_cli_value(args, index, flag)?));
            }
            "--skill-id" => {
                let value = read_cli_value(args, index, flag)?;
                validate_luaskills_identifier(value, "--skill-id")?;
                self.skill_id = Some(value.to_string());
            }
            "--include" => self
                .include
                .push(read_cli_value(args, index, flag)?.to_string()),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Convert parsed options into runtime source options.
    /// 将已解析选项转换为运行时源选项。
    fn into_options(self, command: &str) -> Result<SkillSourceOptions, String> {
        let skill_dir = self
            .skill_path
            .ok_or_else(|| format!("{command} requires --skill-path"))?;
        Ok(SkillSourceOptions {
            skill_dir,
            skill_id: self.skill_id,
            extra_root_names: self.include,
        })
    }
}

/// Parsed `validate` command.
/// 已解析的 `validate` 命令。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidateCommand {
    /// Selected skill source.
    /// 选定的技能源。
    source: SkillSourceOptions,
    /// Output rendering mode.
    /// 输出渲染模式。
    output_mode: DebugOutputMode,
}

/// Parsed `package` command.
/// 已解析的 `package` 命令。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageCommand {
    /// Build request handed to the runtime packager.
    /// 交给运行时打包器的构建请求。
    request: PackageBuildRequest,
    /// Release tag the skill version must match, such as the pushed Git tag.
    /// 技能版本必须匹配的发布标签，例如推送的 Git 标签。
    expect_tag: Option<String>,
    /// Output rendering mode.
    /// 输出渲染模式。
    output_mode: DebugOutputMode,
}

/// Parsed `new` command.
/// 已解析的 `new` 命令。
#[derive(Debug, Clone, PartialEq, Eq)]
struct NewCommand {
    /// Directory to create; its name becomes the skill id.
    /// 要创建的目录；目录名即技能标识。
    dir: PathBuf,
    /// Packaging layout the template starts with.
    /// 模板初始使用的打包布局。
    layout: PackageLayout,
    /// Display name written to skill.yaml.
    /// 写入 skill.yaml 的显示名称。
    name: Option<String>,
}

/// JSON shape of one `validate` result.
/// 单次 `validate` 结果的 JSON 形状。
#[derive(Debug, Serialize)]
struct ValidateOutput<'a> {
    /// Command name.
    /// 命令名称。
    command: &'static str,
    /// Whether the source has no issue.
    /// 技能源是否没有问题。
    valid: bool,
    /// Runtime validation report.
    /// 运行时校验报告。
    #[serde(flatten)]
    report: &'a SkillValidationReport,
    /// LuaSkills Hub limit warnings; they never fail validation.
    /// LuaSkills Hub 限额警告；它们不会导致校验失败。
    warnings: Vec<String>,
}

/// JSON shape of one `package` result.
/// 单次 `package` 结果的 JSON 形状。
#[derive(Debug, Serialize)]
struct PackageOutput<'a> {
    /// Command name.
    /// 命令名称。
    command: &'static str,
    /// Output directory holding every asset.
    /// 保存全部资产的输出目录。
    out_dir: String,
    /// Runtime build report.
    /// 运行时构建报告。
    #[serde(flatten)]
    report: &'a PackageBuildReport,
    /// LuaSkills Hub limit warnings; they never fail packaging.
    /// LuaSkills Hub 限额警告；它们不会导致打包失败。
    warnings: Vec<String>,
}

/// Run `validate`, `package` or `new` when `args` names one of them; return `None` otherwise.
/// 当 `args` 指定 `validate`、`package` 或 `new` 时执行；否则返回 `None`。
pub(crate) fn run_packaging_command(args: &[String]) -> Option<Result<(), String>> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str)? {
        "validate" => Some(parse_validate(rest).and_then(|command| run_validate(&command))),
        "package" => Some(parse_package(rest).and_then(|command| run_package(&command))),
        "new" => Some(parse_new(rest).and_then(|command| run_new(&command))),
        _ => None,
    }
}

/// Parse one packaging layout name.
/// 解析单个打包布局名称。
fn parse_layout(value: &str) -> Result<PackageLayout, String> {
    match value.trim() {
        "standard" => Ok(PackageLayout::Standard),
        "platform" => Ok(PackageLayout::Platform),
        other => Err(format!(
            "Unknown layout '{other}'; expected standard or platform"
        )),
    }
}

/// Return the stable name of one packaging layout.
/// 返回单个打包布局的稳定名称。
fn layout_name(layout: PackageLayout) -> &'static str {
    match layout {
        PackageLayout::Standard => "standard",
        PackageLayout::Platform => "platform",
    }
}

/// Reject the content output mode, which only applies to tool calls.
/// 拒绝仅适用于工具调用的 content 输出模式。
fn parse_report_output(command: &str, value: &str) -> Result<DebugOutputMode, String> {
    match DebugOutputMode::parse(value)? {
        DebugOutputMode::Content => Err(format!("{command} does not support --output content")),
        mode => Ok(mode),
    }
}

/// Parse `validate` options.
/// 解析 `validate` 选项。
fn parse_validate(args: &[String]) -> Result<ValidateCommand, String> {
    let mut source = SourceArgs {
        skill_path: None,
        skill_id: None,
        include: Vec::new(),
    };
    let mut output_mode = DebugOutputMode::Pretty;
    let mut index = 0;
    while index < args.len() {
        if !source.take(args, &mut index)? {
            let flag = args[index].as_str();
            match flag {
                "--output" => {
                    output_mode =
                        parse_report_output("validate", read_cli_value(args, &mut index, flag)?)?;
                }
                other => return Err(format!("Unknown validate option '{other}'")),
            }
        }
        index += 1;
    }
    Ok(ValidateCommand {
        source: source.into_options("validate")?,
        output_mode,
    })
}

/// Parse `package` options.
/// 解析 `package` 选项。
fn parse_package(args: &[String]) -> Result<PackageCommand, String> {
    let mut source = SourceArgs {
        skill_path: None,
        skill_id: None,
        include: Vec::new(),
    };
    let mut out_dir = None;
    let mut layout = PackageLayout::Standard;
    let mut include_universal = true;
    let mut expect_tag = None;
    let mut output_mode = DebugOutputMode::Pretty;
    let mut index = 0;
    while index < args.len() {
        if !source.take(args, &mut index)? {
            let flag = args[index].as_str();
            match flag {
                "--out-dir" => {
                    out_dir = Some(PathBuf::from(read_cli_value(args, &mut index, flag)?))
                }
                "--layout" => layout = parse_layout(read_cli_value(args, &mut index, flag)?)?,
                "--no-universal" => include_universal = false,
                "--expect-tag" => {
                    let value = read_cli_value(args, &mut index, flag)?.trim();
                    if !value.is_empty() {
                        expect_tag = Some(value.to_string());
                    }
                }
                "--output" => {
                    output_mode =
                        parse_report_output("package", read_cli_value(args, &mut index, flag)?)?;
                }
                other => return Err(format!("Unknown package option '{other}'")),
            }
        }
        index += 1;
    }
    if !include_universal && layout == PackageLayout::Standard {
        return Err("--no-universal requires --layout platform".to_string());
    }
    Ok(PackageCommand {
        request: PackageBuildRequest {
            source: source.into_options("package")?,
            out_dir: out_dir.ok_or_else(|| "package requires --out-dir".to_string())?,
            layout,
            include_universal,
        },
        expect_tag,
        output_mode,
    })
}

/// Parse `new` options.
/// 解析 `new` 选项。
fn parse_new(args: &[String]) -> Result<NewCommand, String> {
    let mut dir = None;
    let mut layout = PackageLayout::Standard;
    let mut name = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--layout" => layout = parse_layout(read_cli_value(args, &mut index, flag)?)?,
            "--name" => {
                let value = read_cli_value(args, &mut index, flag)?.trim();
                if value.is_empty() || value.contains(['\r', '\n']) {
                    return Err("--name must be one non-empty line".to_string());
                }
                name = Some(value.to_string());
            }
            other if other.starts_with("--") => {
                return Err(format!("Unknown new option '{other}'"));
            }
            other => {
                if dir.replace(PathBuf::from(other)).is_some() {
                    return Err("new accepts exactly one directory".to_string());
                }
            }
        }
        index += 1;
    }
    Ok(NewCommand {
        dir: dir.ok_or_else(|| {
            "new requires a directory, such as luaskills-debug new my-skill".to_string()
        })?,
        layout,
        name,
    })
}

/// Collect LuaSkills Hub limit warnings for one validated source.
/// 为单个已校验技能源收集 LuaSkills Hub 限额警告。
fn expanded_size_warnings(report: &SkillValidationReport) -> Vec<String> {
    if report.total_bytes > HUB_EXPANDED_LIMIT_BYTES {
        vec![format!(
            "Packaged files total {} bytes, above the LuaSkills Hub expanded limit of {} bytes",
            report.total_bytes, HUB_EXPANDED_LIMIT_BYTES
        )]
    } else {
        Vec::new()
    }
}

/// Collect LuaSkills Hub limit warnings for one built release.
/// 为单个已构建发布收集 LuaSkills Hub 限额警告。
fn package_size_warnings(report: &PackageBuildReport) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut total = 0;
    for asset in &report.assets {
        if !matches!(
            asset.kind,
            PackageAssetKind::Universal | PackageAssetKind::Platform
        ) {
            continue;
        }
        total += asset.size;
        if asset.size > HUB_PACKAGE_LIMIT_BYTES {
            warnings.push(format!(
                "{} is {} bytes, above the LuaSkills Hub package limit of {} bytes",
                asset.name, asset.size, HUB_PACKAGE_LIMIT_BYTES
            ));
        }
    }
    if total > HUB_VERSION_LIMIT_BYTES {
        warnings.push(format!(
            "Packages total {total} bytes, above the LuaSkills Hub per-version limit of {HUB_VERSION_LIMIT_BYTES} bytes"
        ));
    }
    warnings
}

/// Accept a tag equal to the version, with or without one leading `v`.
/// 接受等于版本号的标签，可带或不带一个前导 `v`。
fn tag_matches_version(tag: &str, version: &str) -> bool {
    tag.strip_prefix('v').unwrap_or(tag) == version
}

/// Print a JSON value with a trailing newline.
/// 输出带结尾换行的 JSON 值。
fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Failed to render JSON output: {error}"))?;
    println!("{text}");
    Ok(())
}

/// Print warnings to standard error in pretty mode.
/// 在 pretty 模式下将警告输出到标准错误。
fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}

/// Execute `validate`; issues make the command fail after the report is printed.
/// 执行 `validate`；存在问题时先输出报告再使命令失败。
fn run_validate(command: &ValidateCommand) -> Result<(), String> {
    let report = validate_skill_source(&command.source)?;
    let warnings = expanded_size_warnings(&report);
    match command.output_mode {
        DebugOutputMode::Json => print_json(&ValidateOutput {
            command: "validate",
            valid: report.is_valid(),
            report: &report,
            warnings: warnings.clone(),
        })?,
        _ => {
            println!("skill_id: {}", report.skill_id);
            println!(
                "version: {}",
                report.version.as_deref().unwrap_or("(missing)")
            );
            println!(
                "files: {} ({} bytes)",
                report.file_count, report.total_bytes
            );
            if report.bundled_platforms.is_empty() {
                println!("bundled platforms: none (standard layout)");
            } else {
                println!(
                    "bundled platforms: {} (platform layout available)",
                    report.bundled_platforms.join(", ")
                );
            }
            for issue in &report.issues {
                match &issue.path {
                    Some(path) => println!("issue [{}] {}: {}", issue.code, path, issue.message),
                    None => println!("issue [{}] {}", issue.code, issue.message),
                }
            }
            print_warnings(&warnings);
            if report.is_valid() {
                println!("valid");
            }
        }
    }
    if report.is_valid() {
        Ok(())
    } else {
        Err(format!("{} validation issue(s) found", report.issues.len()))
    }
}

/// Execute `package`.
/// 执行 `package`。
fn run_package(command: &PackageCommand) -> Result<(), String> {
    if let Some(tag) = &command.expect_tag {
        let report = validate_skill_source(&command.request.source)?;
        let version = report.version.as_deref().unwrap_or_default();
        if !tag_matches_version(tag, version) {
            return Err(format!(
                "Release tag '{tag}' does not match skill.yaml version '{version}'"
            ));
        }
    }
    let report = build_skill_release(&command.request)?;
    let warnings = package_size_warnings(&report);
    match command.output_mode {
        DebugOutputMode::Json => print_json(&PackageOutput {
            command: "package",
            out_dir: render_host_visible_path(&command.request.out_dir),
            report: &report,
            warnings,
        }),
        _ => {
            println!(
                "{} v{} ({} layout)",
                report.skill_id,
                report.version,
                layout_name(report.layout)
            );
            for asset in &report.assets {
                println!("  {}  {} bytes  {}", asset.sha256, asset.size, asset.name);
            }
            println!(
                "out_dir: {}",
                render_host_visible_path(&command.request.out_dir)
            );
            print_warnings(&warnings);
            Ok(())
        }
    }
}

/// Replace template placeholders for one new skill.
/// 为单个新技能替换模板占位符。
fn render_template(text: &str, skill_id: &str, name: &str, layout: PackageLayout) -> String {
    text.replace("{{SKILL_ID}}", skill_id)
        .replace("{{SKILL_NAME}}", name)
        .replace("{{LIB_STEM}}", &skill_id.replace('-', "_"))
        .replace("{{LAYOUT}}", layout_name(layout))
        .replace("{{DEBUG_VERSION}}", env!("CARGO_PKG_VERSION"))
}

/// Turn one skill id into a display name, such as `my-skill` into `My Skill`.
/// 将技能标识转换为显示名称，例如 `my-skill` 转为 `My Skill`。
fn default_display_name(skill_id: &str) -> String {
    skill_id
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Write one new skill from the embedded template; an existing non-empty directory is refused.
/// 从内嵌模板写入单个新技能；拒绝已存在的非空目录。
fn write_new_skill(command: &NewCommand) -> Result<String, String> {
    let skill_id = command
        .dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "new requires a directory with a UTF-8 name".to_string())?
        .to_string();
    validate_luaskills_identifier(&skill_id, "new skill directory name")?;
    if command.dir.exists() {
        let mut entries = fs::read_dir(&command.dir).map_err(|error| {
            format!(
                "Failed to read {}: {error}",
                render_host_visible_path(&command.dir)
            )
        })?;
        if entries.next().is_some() {
            return Err(format!(
                "{} already exists and is not empty",
                render_host_visible_path(&command.dir)
            ));
        }
    }
    let name = command
        .name
        .clone()
        .unwrap_or_else(|| default_display_name(&skill_id));
    let platform_files = match command.layout {
        PackageLayout::Standard => &[][..],
        PackageLayout::Platform => PLATFORM_TEMPLATE_FILES,
    };
    for file in COMMON_TEMPLATE_FILES.iter().chain(platform_files) {
        let path = command.dir.join(file.path);
        write_template_file(
            &path,
            &render_template(file.text, &skill_id, &name, command.layout),
        )?;
    }
    Ok(skill_id)
}

/// Write one rendered template file and its parent directories.
/// 写入单个已渲染模板文件及其父目录。
fn write_template_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create {}: {error}",
                render_host_visible_path(parent)
            )
        })?;
    }
    fs::write(path, text).map_err(|error| {
        format!(
            "Failed to write {}: {error}",
            render_host_visible_path(path)
        )
    })
}

/// Execute `new`.
/// 执行 `new`。
fn run_new(command: &NewCommand) -> Result<(), String> {
    let skill_id = write_new_skill(command)?;
    let dir = render_host_visible_path(&command.dir);
    println!(
        "Created {skill_id} ({} layout) in {dir}",
        layout_name(command.layout)
    );
    if command.layout == PackageLayout::Platform {
        println!(
            "Put the compiled libraries listed in native/README.md under native/ before packaging."
        );
    }
    println!("Next:");
    println!("  luaskills-debug validate --skill-path {dir}");
    println!(
        "  luaskills-debug call --runtime-root {dir}/.luaskills-debug --skill-path {dir} --tool hello --args-file {dir}/examples/hello.args.json"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
