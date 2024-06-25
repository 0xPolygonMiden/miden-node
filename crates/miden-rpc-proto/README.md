# Miden node utils

This crate contains the RPC protobuf message definitions.
It consists of a map of `(filename, file contents)` where each entry refers to a protobuf file.

Additionally, the crate exposes a `write_proto(target_dir)` function that writes the files into `target_dir`.

## License
This project is [MIT licensed](../../LICENSE).
