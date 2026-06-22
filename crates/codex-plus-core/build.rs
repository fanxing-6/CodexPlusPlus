use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CODEX_FLAMESHOT_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=CODEX_FLAMESHOT_CMAKE_BUILD_TYPE");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMBEDDED_FLAMESHOT");

    let Some(workspace_dir) = workspace_dir() else {
        return;
    };
    let native_dir = workspace_dir.join("native").join("flameshot-embedded");
    println!("cargo:rerun-if-changed={}", native_dir.display());

    if env::var_os("CARGO_FEATURE_EMBEDDED_FLAMESHOT").is_none() {
        return;
    }

    let source_dir = env::var_os("CODEX_FLAMESHOT_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            workspace_dir
                .join("dist")
                .join("vendor")
                .join("flameshot-src")
        });
    if !source_dir
        .join("src")
        .join("core")
        .join("flameshot.h")
        .is_file()
    {
        panic!(
            "embedded-flameshot requires Flameshot source at {}. Run scripts/vendor/prepare-flameshot-source.sh or set CODEX_FLAMESHOT_SOURCE_DIR.",
            source_dir.display()
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let build_dir = out_dir.join("flameshot-embedded-build");
    let lib_dir = out_dir.join("flameshot-embedded-lib");
    let bin_dir = out_dir.join("flameshot-embedded-bin");
    std::fs::create_dir_all(&build_dir).expect("failed to create embedded Flameshot build dir");
    std::fs::create_dir_all(&lib_dir).expect("failed to create embedded Flameshot lib dir");
    std::fs::create_dir_all(&bin_dir).expect("failed to create embedded Flameshot bin dir");

    let cmake_build_type =
        env::var("CODEX_FLAMESHOT_CMAKE_BUILD_TYPE").unwrap_or_else(|_| "Release".to_string());

    run(
        Command::new("cmake")
            .arg("-S")
            .arg(&native_dir)
            .arg("-B")
            .arg(&build_dir)
            .arg(format!("-DCMAKE_BUILD_TYPE={cmake_build_type}"))
            .arg(format!("-DFLAMESHOT_SOURCE_DIR={}", source_dir.display()))
            .arg(format!(
                "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY={}",
                lib_dir.display()
            ))
            .arg(format!(
                "-DCMAKE_LIBRARY_OUTPUT_DIRECTORY={}",
                lib_dir.display()
            ))
            .arg(format!(
                "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY={}",
                bin_dir.display()
            ))
            .arg(format!(
                "-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE={}",
                lib_dir.display()
            ))
            .arg(format!(
                "-DCMAKE_LIBRARY_OUTPUT_DIRECTORY_RELEASE={}",
                lib_dir.display()
            ))
            .arg(format!(
                "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE={}",
                bin_dir.display()
            ))
            .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON"),
        "configure embedded Flameshot",
    );
    run(
        Command::new("cmake")
            .arg("--build")
            .arg(&build_dir)
            .arg("--config")
            .arg(&cmake_build_type),
        "build embedded Flameshot",
    );

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", bin_dir.display());
    println!("cargo:rustc-link-lib=dylib=codex_flameshot_embedded");
}

fn workspace_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    manifest_dir.parent()?.parent().map(Path::to_path_buf)
}

fn run(command: &mut Command, description: &str) {
    let status = command.status().unwrap_or_else(|error| {
        panic!("{description} failed to start: {error}");
    });
    if !status.success() {
        panic!("{description} failed with status {status}");
    }
}
