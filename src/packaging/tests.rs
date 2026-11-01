use super::*;
use luaskills::package::{PackageAsset, read_package_marker};

/// Create one empty temporary directory unique to the calling test.
/// 为调用测试创建一个唯一的空临时目录。
fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "luaskills_debug_packaging_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

/// Convert string literals into one owned argument vector.
/// 将字符串字面量转换为一个自有参数向量。
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// Build one fake package asset of the given kind and size.
/// 构建一个指定类型与大小的伪包资产。
fn asset(name: &str, kind: PackageAssetKind, size: u64) -> PackageAsset {
    PackageAsset {
        name: name.to_string(),
        kind,
        platform: None,
        sha256: "0".repeat(64),
        size,
        path: PathBuf::from(name),
    }
}

#[test]
fn package_options_default_to_standard_and_reject_bad_combinations() {
    let command =
        parse_package(&args(&["--skill-path", "demo", "--out-dir", "dist"])).expect("parse");
    assert_eq!(command.request.layout, PackageLayout::Standard);
    assert!(command.request.include_universal);
    assert_eq!(command.output_mode, DebugOutputMode::Pretty);

    let command = parse_package(&args(&[
        "--skill-path",
        "demo",
        "--out-dir",
        "dist",
        "--layout",
        "platform",
        "--no-universal",
        "--include",
        "rules",
        "--expect-tag",
        "v1.0.0",
        "--output",
        "json",
    ]))
    .expect("parse platform");
    assert_eq!(command.request.layout, PackageLayout::Platform);
    assert!(!command.request.include_universal);
    assert_eq!(command.request.source.extra_root_names, ["rules"]);
    assert_eq!(command.expect_tag.as_deref(), Some("v1.0.0"));

    assert!(
        parse_package(&args(&[
            "--skill-path",
            "demo",
            "--out-dir",
            "dist",
            "--no-universal"
        ]))
        .is_err()
    );
    assert!(parse_package(&args(&["--skill-path", "demo"])).is_err());
    assert!(parse_package(&args(&["--out-dir", "dist"])).is_err());
    assert!(
        parse_package(&args(&[
            "--skill-path",
            "demo",
            "--out-dir",
            "dist",
            "--layout",
            "split"
        ]))
        .is_err()
    );
    assert!(
        parse_package(&args(&[
            "--skill-path",
            "demo",
            "--out-dir",
            "dist",
            "--output",
            "content"
        ]))
        .is_err()
    );
    assert!(parse_validate(&args(&["--skill-path", "demo", "--skill-id", "Bad_Id"])).is_err());
}

#[test]
fn tags_match_versions_with_an_optional_v_prefix() {
    assert!(tag_matches_version("v1.2.3", "1.2.3"));
    assert!(tag_matches_version("1.2.3", "1.2.3"));
    assert!(!tag_matches_version("vv1.2.3", "1.2.3"));
    assert!(!tag_matches_version("v1.2.4", "1.2.3"));
}

#[test]
fn size_warnings_follow_hub_limits() {
    let report = PackageBuildReport {
        skill_id: "demo".to_string(),
        version: "1.0.0".to_string(),
        layout: PackageLayout::Platform,
        assets: vec![
            asset(
                "small.zip",
                PackageAssetKind::Platform,
                HUB_PACKAGE_LIMIT_BYTES,
            ),
            asset(
                "index.json",
                PackageAssetKind::Index,
                HUB_VERSION_LIMIT_BYTES * 2,
            ),
        ],
    };
    assert!(package_size_warnings(&report).is_empty());

    let report = PackageBuildReport {
        assets: (0..7)
            .map(|index| {
                asset(
                    &format!("p{index}.zip"),
                    PackageAssetKind::Platform,
                    HUB_PACKAGE_LIMIT_BYTES + 1,
                )
            })
            .collect(),
        ..report
    };
    // Seven oversized packages warn once each and once more for the version total.
    // 七个超限包各警告一次，版本合计再警告一次。
    assert_eq!(package_size_warnings(&report).len(), 8);
}

#[test]
fn new_standard_skill_validates_and_packages() {
    let root = temp_root("standard");
    let dir = root.join("my-skill");
    parse_new(&args(&[dir.to_str().expect("utf-8 path")]))
        .and_then(|command| write_new_skill(&command))
        .expect("write template");

    let skill_yaml = fs::read_to_string(dir.join("skill.yaml")).expect("read skill.yaml");
    assert!(skill_yaml.starts_with("name: My Skill\n"), "{skill_yaml}");
    let workflow =
        fs::read_to_string(dir.join(".github/workflows/release.yml")).expect("read workflow");
    assert!(
        !workflow.contains("{{SKILL_")
            && !workflow.contains("{{LAYOUT}}")
            && !workflow.contains("{{DEBUG_VERSION}}")
    );
    assert!(!dir.join("dependencies.yaml").exists());

    let source = SkillSourceOptions {
        skill_dir: dir.clone(),
        skill_id: None,
        extra_root_names: Vec::new(),
    };
    let report = validate_skill_source(&source).expect("validate");
    assert!(report.is_valid(), "{:?}", report.issues);
    assert_eq!(report.skill_id, "my-skill");

    let build = build_skill_release(&PackageBuildRequest {
        source,
        out_dir: root.join("dist"),
        layout: PackageLayout::Standard,
        include_universal: true,
    })
    .expect("package");
    let names: Vec<&str> = build
        .assets
        .iter()
        .map(|asset| asset.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["my-skill-v0.1.0-skill.zip", "my-skill-v0.1.0-checksums.txt"]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn new_platform_skill_reports_missing_libraries_until_they_exist() {
    let root = temp_root("platform");
    let dir = root.join("native-skill");
    let command = parse_new(&args(&[
        dir.to_str().expect("utf-8 path"),
        "--layout",
        "platform",
        "--name",
        "Native",
    ]))
    .expect("parse");
    write_new_skill(&command).expect("write template");

    let source = SkillSourceOptions {
        skill_dir: dir.clone(),
        skill_id: None,
        extra_root_names: Vec::new(),
    };
    let report = validate_skill_source(&source).expect("validate");
    assert!(!report.is_valid());
    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.code == "bundled_payload_missing"),
        "{:?}",
        report.issues
    );

    for (platform, file) in [
        ("windows-x64", "native_skill.dll"),
        ("linux-x64", "libnative_skill.so"),
        ("linux-arm64", "libnative_skill.so"),
        ("macos-x64", "libnative_skill.dylib"),
        ("macos-arm64", "libnative_skill.dylib"),
    ] {
        write_template_file(&dir.join("native").join(platform).join(file), platform)
            .expect("write library");
    }
    let report = validate_skill_source(&source).expect("validate again");
    assert!(report.is_valid(), "{:?}", report.issues);
    assert_eq!(report.bundled_platforms.len(), 5);

    let build = build_skill_release(&PackageBuildRequest {
        source,
        out_dir: root.join("dist"),
        layout: PackageLayout::Platform,
        include_universal: false,
    })
    .expect("package");
    assert!(
        build
            .assets
            .iter()
            .any(|asset| asset.kind == PackageAssetKind::Index)
    );
    assert!(
        !build
            .assets
            .iter()
            .any(|asset| asset.kind == PackageAssetKind::Universal)
    );
    assert_eq!(read_package_marker(&dir).expect("read source marker"), None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn new_refuses_non_empty_directories_and_invalid_ids() {
    let root = temp_root("refuse");
    let dir = root.join("taken");
    write_template_file(&dir.join("keep.txt"), "keep").expect("seed");
    let command = parse_new(&args(&[dir.to_str().expect("utf-8 path")])).expect("parse");
    assert!(write_new_skill(&command).is_err());
    assert_eq!(
        fs::read_to_string(dir.join("keep.txt")).expect("kept"),
        "keep"
    );

    let command = parse_new(&args(&[root
        .join("Bad_Name")
        .to_str()
        .expect("utf-8 path")]))
    .expect("parse");
    assert!(write_new_skill(&command).is_err());
    assert!(parse_new(&args(&["a", "b"])).is_err());
    assert!(parse_new(&args(&[])).is_err());
    let _ = fs::remove_dir_all(root);
}
