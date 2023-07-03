// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let protos = &[
//         "proto/hash.proto",
//         "proto/account.proto",
//         "proto/trees.proto",
//     ];
//     let includes = &["proto"];
//     tonic_build::configure()
//         .build_server(false)
//         .compile(protos, includes)?;
//     Ok(())
//  }

use miette::IntoDiagnostic;
use prost::Message;
use std::{fs, path::PathBuf};

fn main() -> miette::Result<()> {
    let protos = &[
        "proto/hash.proto",
        "proto/account.proto",
        "proto/trees.proto",
    ];
    let includes = &["proto"];
    let file_descriptors = protox::compile(protos, includes)?;

    let file_descriptor_path = PathBuf::from("src").join("file_descriptor_set.bin");
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec()).into_diagnostic()?;

    tonic_build::configure()
        .file_descriptor_set_path(&file_descriptor_path)
        .skip_protoc_run()
        .compile(protos, includes)
        .into_diagnostic()?;

    Ok(())
}
