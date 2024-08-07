use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

// CONSTANTS
// ================================================================================================

const REPO_PROTO_DIR: &str = "../../proto";
const CRATE_PROTO_DIR: &str = "proto";

const DOC_COMMENT: &str =
    "A list of tuples containing the names and contents of various protobuf files.";

// BUILD SCRIPT
// ================================================================================================

/// Copies .proto files to the local directory and re-builds src/proto_files.rs file.
///
/// This is done only if BUILD_PROTO environment variable is set to `1` to avoid running the script
/// on crates.io where repo-level .proto files are not available.
fn main() -> io::Result<()> {
    println!("cargo::rerun-if-changed=../../proto");
    println!("cargo::rerun-if-env-changed=BUILD_PROTO");

    // skip this build script in BUILD_PROTO environment variable is not set to `1`
    if env::var("BUILD_PROTO").unwrap_or("0".to_string()) == "0" {
        return Ok(());
    }

    // Copy all .proto files into this crate. all these files need to be local to the crate to
    // publish the crate to crates.io
    fs::remove_dir_all(CRATE_PROTO_DIR)?;
    fs::create_dir(CRATE_PROTO_DIR)?;
    copy_proto_files()?;

    let out_dir = env::current_dir().expect("Error getting cwd");
    let dest_path = Path::new(&out_dir).join("./src/proto_files.rs");
    fs::remove_file(&dest_path)?;

    let mut proto_filenames = Vec::new();
    for entry in fs::read_dir(CRATE_PROTO_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name =
                path.file_name().and_then(|f| f.to_str()).expect("Could not get file name");

            proto_filenames.push(format!(
                "    (\"{file_name}\", include_str!(\"../{CRATE_PROTO_DIR}/{file_name}\")),\n"
            ));
        }
    }
    // Sort so that the vector is consistent since directory walking order is
    // not guaranteed, otherwise there will be diffs from different runs.
    proto_filenames.sort();

    let content = std::iter::once(format!(
        "/// {DOC_COMMENT}\npub const PROTO_FILES: &[(&str, &str)] = &[\n"
    ))
    .chain(proto_filenames)
    .chain(std::iter::once("];\n".to_string()))
    .collect::<String>();
    fs::write(dest_path, content)?;

    Ok(())
}

// HELPER FUNCTIONS
// ================================================================================================

/// Copies all .proto files from the root proto directory to the proto directory of this crate.
fn copy_proto_files() -> io::Result<()> {
    let dest_dir: PathBuf = CRATE_PROTO_DIR.into();

    fs::create_dir_all(dest_dir.clone())?;
    for entry in fs::read_dir(REPO_PROTO_DIR)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if !ty.is_dir() {
            fs::copy(entry.path(), dest_dir.join(entry.file_name()))?;
        }
    }

    Ok(())
}
