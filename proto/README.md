# Miden proto build

This crate contains the proto definitions for the gRPC endpoints at `proto/proto`.

Also exposes the compiled proto `FileDescriptorSet` to generate the Rust bindings. See next how to use it.

# Usage
Likely, you will need to set up a `build.rs` file that generates the proto bindings at build time. How use the proto builder:

```rust
use std::{fs, path::PathBuf};
use miden_node_proto_build::rpc_file_descriptor;

fn main() {
    let dst_dir = PathBuf::from("./generated");

    fs::remove_dir_all(&dst_dir).unwrap();
    fs::create_dir(&dst_dir).unwrap();

    let file_descriptors = rpc_file_descriptor().unwrap();

    let mut prost_config = prost_build::Config::new();
    prost_config.skip_debug(["AccountId", "Digest"]);

    tonic_build::configure()
        .out_dir(dst_dir)
        .compile_fds_with_config(prost_config, file_descriptors)
        .context("compiling protobufs")?;
}
```

# Features

- "internal": when set, the functions `store_file_descriptor()` and `block_producer_file_descriptor()` are exposed, that can be used to generate all the bindings needed for the node.

## License
This project is [MIT licensed](../../LICENSE).
