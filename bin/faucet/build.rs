use std::str::FromStr;

fn main() -> std::io::Result<()> {
    // The location of our static faucet website files.
    let static_dir = std::path::PathBuf::from_str(std::env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .join("src")
        .join("static");
    println!("cargo::rerun-if-changed={}", static_dir.to_str().expect("Valid utf-8"));
    // This makes the static files available as an embedded resource.
    static_files::resource_dir(static_dir).build()
}
