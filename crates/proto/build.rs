use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use miden_node_proto_build::ProtoBuilder;

/// Generates Rust protobuf bindings from .proto files in the root directory.
///
/// This is done only if `BUILD_PROTO` environment variable is set to `1` to avoid running the
/// script on crates.io where repo-level .proto files are not available.
fn main() -> anyhow::Result<()> {
    println!("cargo::rerun-if-changed=../../proto/proto");
    println!("cargo::rerun-if-env-changed=BUILD_PROTO");

    // Skip this build script in BUILD_PROTO environment variable is not set to `1`.
    if env::var("BUILD_PROTO").unwrap_or("0".to_string()) == "0" {
        return Ok(());
    }

    let crate_root: PathBuf =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set").into();
    let dst_dir = crate_root.join("src").join("generated");

    // Remove all existing files.
    fs::remove_dir_all(&dst_dir).context("removing existing files")?;
    fs::create_dir(&dst_dir).context("creating destination folder")?;

    // Build the proto files in the destination directory.
    let builder = tonic_build::configure().out_dir(&dst_dir);

    ProtoBuilder::new(builder).compile().context("compiling proto files")?;

    generate_mod_rs(&dst_dir).context("generating mod.rs")?;

    Ok(())
}

/// Generate `mod.rs` which includes all files in the folder as submodules.
fn generate_mod_rs(directory: impl AsRef<Path>) -> std::io::Result<()> {
    let mod_filepath = directory.as_ref().join("mod.rs");

    // Discover all submodules by iterating over the folder contents.
    let mut submodules = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_stem = path
                .file_stem()
                .and_then(|f| f.to_str())
                .expect("Could not get file name")
                .to_owned();

            submodules.push(file_stem);
        }
    }

    submodules.sort();

    let contents = submodules.iter().map(|f| format!("pub mod {f};\n"));
    let contents = std::iter::once(
        "#![allow(clippy::pedantic, reason = \"generated by build.rs and tonic\")]\n\n".to_string(),
    )
    .chain(contents)
    .collect::<String>();

    fs::write(mod_filepath, contents)
}
