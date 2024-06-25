/// A mapping of filenames to file contents of the node protobuf files.
pub const PROTO_FILES: &[(&str, &str)] = &[
    ("note.proto", include_str!("../../../proto/note.proto")),
    ("smt.proto", include_str!("../../../proto/smt.proto")),
    ("responses.proto", include_str!("../../../proto/responses.proto")),
    ("rpc.proto", include_str!("../../../proto/rpc.proto")),
    ("store.proto", include_str!("../../../proto/store.proto")),
    ("transaction.proto", include_str!("../../../proto/transaction.proto")),
    ("mmr.proto", include_str!("../../../proto/mmr.proto")),
    ("account.proto", include_str!("../../../proto/account.proto")),
    ("block_header.proto", include_str!("../../../proto/block_header.proto")),
    ("digest.proto", include_str!("../../../proto/digest.proto")),
    ("block_producer.proto", include_str!("../../../proto/block_producer.proto")),
    ("merkle.proto", include_str!("../../../proto/merkle.proto")),
    ("requests.proto", include_str!("../../../proto/requests.proto")),
];
