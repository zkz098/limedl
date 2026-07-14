use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    tauri_build::build();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Write a minimal .rc file containing only the ComCtl32 v6 manifest.
    // Each line between { and } must be a quoted string.  Double-quotes
    // inside must be doubled in .rc syntax (not backslash-escaped).
    let manifest_rc = out_path.join("manifest_only.rc");
    std::fs::write(
        &manifest_rc,
        concat!(
            "#pragma code_page(65001)\n",
            "1 24\n",
            "{\n",
            "\" <assembly xmlns=\"\"urn:schemas-microsoft-com:asm.v1\"\" manifestVersion=\"\"1.0\"\"> \"\n",
            "\" <dependency> \"\n",
            "\" <dependentAssembly> \"\n",
            "\" <assemblyIdentity \"\n",
            "\" type=\"\"win32\"\" \"\n",
            "\" name=\"\"Microsoft.Windows.Common-Controls\"\" \"\n",
            "\" version=\"\"6.0.0.0\"\" \"\n",
            "\" processorArchitecture=\"\"*\"\" \"\n",
            "\" publicKeyToken=\"\"6595b64144ccf1df\"\" \"\n",
            "\" language=\"\"*\"\" \"\n",
            "\" /> \"\n",
            "\" </dependentAssembly> \"\n",
            "\" </dependency> \"\n",
            "\" </assembly> \"\n",
            "}\n",
        ),
    )
    .unwrap();

    // Find MSVC / Windows SDK tools
    let rc = find_tool_in_latest_sdk_version("rc.exe");
    let cvtres = find_tool_in_latest_msvc_version("cvtres.exe");

    if let (Some(rc), Some(cvtres)) = (&rc, &cvtres) {
        let res_file = out_path.join("manifest_only.res");
        let obj_file = out_path.join("manifest_only.obj");

        // rc.exe → .res
        let rc_ok = Command::new(rc)
            .arg("/fo")
            .arg(&res_file)
            .arg(&manifest_rc)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        // cvtres.exe → .obj
        let cvtres_ok = rc_ok
            && {
                let out_arg = format!("/out:{}", obj_file.display());
                Command::new(cvtres)
                    .arg(&out_arg)
                    .arg(&res_file)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            };

        if cvtres_ok {
            // Link the manifest-only .obj to all targets as a direct
            // linker input.  This forces the linker to include the
            // manifest resource even though it doesn't resolve any
            // code symbols (unlike `cargo:rustc-link-lib=static` which
            // only pulls in objects that resolve symbols).
            println!("cargo:rustc-link-arg={}", obj_file.display());
            return;
        }
    }

    // Fallback: link resource.lib.  `cargo test --lib` will work, but
    // `cargo test` / `cargo test --workspace` may fail with duplicate
    // VERSION errors for the binary test target.
    eprintln!(
        "warning: could not build manifest-only obj (rc={}, cvtres={}); \
         using resource.lib fallback",
        rc.is_some(),
        cvtres.is_some(),
    );
    let resource_lib = out_path.join("resource.lib");
    if resource_lib.exists() {
        println!("cargo:rustc-link-arg={}", resource_lib.display());
    }
}

/// Find a tool in the latest Windows SDK version directory.
/// Looks under: `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64\`
fn find_tool_in_latest_sdk_version(name: &str) -> Option<PathBuf> {
    let kits_bin = Path::new("C:\\Program Files (x86)\\Windows Kits\\10\\bin");
    if !kits_bin.exists() {
        return None;
    }
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits_bin)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // Sort by version (lexicographic sort works for numeric version strings)
    versions.sort();
    // Pick the highest version
    while let Some(ver_dir) = versions.pop() {
        let candidate = ver_dir.join("x64").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Find a tool in the latest MSVC version directory.
/// Looks under: `C:\Program Files\Microsoft Visual Studio\<year>\<edition>\VC\Tools\MSVC\<version>\bin\Hostx64\x64\`
fn find_tool_in_latest_msvc_version(name: &str) -> Option<PathBuf> {
    let vs_roots = [
        Path::new("C:\\Program Files\\Microsoft Visual Studio\\18"),
        Path::new("C:\\Program Files (x86)\\Microsoft Visual Studio\\2019"),
    ];

    for vs_root in &vs_roots {
        if !vs_root.exists() {
            continue;
        }
        // Find edition directories (Community, Professional, Enterprise)
        let editions: Vec<PathBuf> = std::fs::read_dir(vs_root)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();

        for edition in &editions {
            let msvc_root = edition.join("VC\\Tools\\MSVC");
            if !msvc_root.exists() {
                continue;
            }
            let mut versions: Vec<PathBuf> = std::fs::read_dir(&msvc_root)
                .ok()?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            // Try from newest to oldest
            while let Some(ver_dir) = versions.pop() {
                // Try both Hostx64 and HostX64 (case varies between MSVC versions)
                for host_dir in &["Hostx64", "HostX64"] {
                    let candidate = ver_dir.join("bin").join(host_dir).join("x64").join(name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}
