use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_clang_runtime_for_metal_availability_checks();
        build_media_adapter();
    }
    tauri_build::build()
}

fn build_media_adapter() {
    println!("cargo:rerun-if-changed=../scripts/build-media-adapter.sh");
    println!("cargo:rerun-if-changed=../vendor/mediaremote-adapter");
    let status = Command::new("sh").arg("../scripts/build-media-adapter.sh").status();
    assert!(
        status.is_ok_and(|status| status.success()),
        "building the media adapter from vendor/mediaremote-adapter failed"
    );
}

fn link_clang_runtime_for_metal_availability_checks() {
    let Ok(output) = Command::new("clang").arg("--print-runtime-dir").output() else {
        return;
    };
    let runtime_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if std::path::Path::new(&runtime_dir).join("libclang_rt.osx.a").exists() {
        println!("cargo:rustc-link-search=native={runtime_dir}");
        println!("cargo:rustc-link-lib=static=clang_rt.osx");
    }
}
