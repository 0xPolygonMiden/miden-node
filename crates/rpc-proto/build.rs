use std::{
    env,
    fs::{self, File},
    io::{self, Read, Write},
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
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=../../proto");

    // skip this build script in BUILD_PROTO environment variable is not set to `1`
    if env::var("BUILD_PROTO").unwrap_or("0".to_string()) == "0" {
        return Ok(())
    }

    // copy all .proto files into this crate. all these files need to be local to the crate to
    // publish the crate to crates.io
    copy_proto_files()?;

    let out_dir = env::current_dir().expect("Error getting cwd");
    let dest_path = Path::new(&out_dir).join("./src/proto_files.rs");
    let mut file = File::create(dest_path)?;

    writeln!(file, "/// {DOC_COMMENT}")?;
    writeln!(file, "pub const PROTO_FILES: &[(&str, &str)] = &[")?;

    for entry in std::fs::read_dir(CRATE_PROTO_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let mut file_content = String::new();
            let file_name =
                path.file_name().and_then(|f| f.to_str()).expect("Could not get file name");

            File::open(&path)?.read_to_string(&mut file_content)?;
            writeln!(
                file,
                "    (\"{}\", include_str!(\"../{CRATE_PROTO_DIR}/{}\")),",
                file_name, file_name
            )?;
        }
    }

    writeln!(file, "];")?;

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
        println!("{entry:?}");
        let ty = entry.file_type()?;
        if !ty.is_dir() {
            fs::copy(entry.path(), dest_dir.join(entry.file_name()))?;
        }
    }

    Ok(())
}