# Miden proto build

This crate contains the proto definitions for the gRPC endpoints at `proto/proto`.

Also exposes a [`ProtoBuilder`](src/lib.rs#L10) for generating the Rust bindings. See next how to use it.

# Usage
Likely, you will need to set up a `build.rs` file that generates the proto bindings at build time. How use the proto builder:

```rust
use std::{fs, path::PathBuf};
use miden_node_proto_build::ProtoBuilder;

fn main() {
    let dst_dir = PathBuf::from("./generated");

    fs::remove_dir_all(&dst_dir).unwrap();
    fs::create_dir(&dst_dir).unwrap();

    let builder = tonic_build::configure().out_dir(&dst_dir);

    ProtoBuilder::new(builder).compile().unwrap();
}
```

By default, the generated files don't contain the RPC Client component. For that, you should use miden-client's [TonicRpcClient](https://github.com/0xPolygonMiden/miden-client/blob/f99b7e0b68dfd05f281981f47b0ce7972f8cdb67/crates/rust-client/src/rpc/tonic_client/mod.rs#L41-L51).

If you still need the RPC ApiClient, you can generate it by setting the feature flag "rpc-client".

## License
This project is [MIT licensed](../../LICENSE).
