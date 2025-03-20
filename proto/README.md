# Miden proto build

This crate contains the code for implementing builders to generate proto bindings. It accesses the proto definitions that are placed in the workspace and exposes these builders to allow for bindings generation.

# Usage
How use the proto builder:

```rust
use std::{fs, path::PathBuf};
use miden_node_proto_build::ProtoBuilder;

fn main() {
    let dst_dir = PathBuf::from("./generated");

    fs::remove_dir_all(&dst_dir).unwrap();
    fs::create_dir(&dst_dir).unwrap();

    let builder = tonic_build::configure().out_dir(&dst_dir);

    ProtoBuilder::new(builder).compile_rpc_client().unwrap()
}
```

## License
This project is [MIT licensed](../../LICENSE).
