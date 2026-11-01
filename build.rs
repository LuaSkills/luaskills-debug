//! Embed the Windows application manifest that opts the debugger into UTF-8 Win32 paths.
//! 嵌入 Windows 应用清单，使调试工具启用 UTF-8 Win32 路径。

use std::{env, path::PathBuf};

/// Link the manifest only for MSVC Windows targets; other targets need no build step.
/// 仅为 MSVC Windows 目标链接清单；其他目标无需构建步骤。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=luaskills-debug.manifest");
    if env::var("CARGO_CFG_TARGET_OS")? == "windows" && env::var("CARGO_CFG_TARGET_ENV")? == "msvc"
    {
        let manifest =
            PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("luaskills-debug.manifest");
        println!("cargo::rustc-link-arg-bin=luaskills-debug=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg-bin=luaskills-debug=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    Ok(())
}
