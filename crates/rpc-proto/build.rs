use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

fn main() -> io::Result<()> {
    let out_dir = env::current_dir().expect("Error getting cwd");
    let dest_path = Path::new(&out_dir).join("./src/proto_files.rs");
    let mut file = File::create(&dest_path)?;

    let proto_dir = Path::new("../../proto");

    writeln!(file, "pub const PROTO_FILES: &[(&str, &str)] = &[")?;

    for entry in std::fs::read_dir(proto_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let mut file_content = String::new();
            let file_name =
                path.file_name().and_then(|f| f.to_str()).expect("Could not get file name");

            File::open(&path)?.read_to_string(&mut file_content)?;
            writeln!(
                file,
                "    (\"{}\", include_str!(\"../../../proto/{}\")),",
                file_name, file_name
            )?;
        }
    }

    writeln!(file, "];")?;

    Ok(())
}
