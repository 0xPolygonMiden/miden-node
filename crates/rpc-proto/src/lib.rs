use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

mod proto_files;
pub use proto_files::PROTO_FILES;

/// Writes the RPC protobuf file into `target_dir`.
pub fn write_proto(target_dir: &Path) -> Result<(), String> {
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)
            .map_err(|err| format!("Error creating directory: {}", err))?;
    } else if !target_dir.is_dir() {
        return Err("The target path exists but is not a directory".to_string());
    }

    for (file_name, file_content) in PROTO_FILES {
        let mut file_path = target_dir.to_path_buf();
        file_path.push(file_name);
        let mut file = File::create(&file_path)
            .map_err(|err| format!("Error creating {}: {}", file_name, err))?;
        file.write_all(file_content.as_bytes())
            .map_err(|err| format!("Error writing {}: {}", file_name, err))?;
    }

    Ok(())
}
