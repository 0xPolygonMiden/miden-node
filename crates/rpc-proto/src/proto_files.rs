/// A list of tuples containing the names and contents of various protobuf files.
pub const PROTO_FILES: &[(&str, &str)] = &[
    ("account.proto", include_str!("../proto/account.proto")),
    ("block.proto", include_str!("../proto/block.proto")),
    ("block_producer.proto", include_str!("../proto/block_producer.proto")),
    ("digest.proto", include_str!("../proto/digest.proto")),
    ("merkle.proto", include_str!("../proto/merkle.proto")),
    ("mmr.proto", include_str!("../proto/mmr.proto")),
    ("network_transaction.proto", include_str!("../proto/network_transaction.proto")),
    ("note.proto", include_str!("../proto/note.proto")),
    ("requests.proto", include_str!("../proto/requests.proto")),
    ("responses.proto", include_str!("../proto/responses.proto")),
    ("rpc.proto", include_str!("../proto/rpc.proto")),
    ("smt.proto", include_str!("../proto/smt.proto")),
    ("store.proto", include_str!("../proto/store.proto")),
    ("transaction.proto", include_str!("../proto/transaction.proto")),
];
