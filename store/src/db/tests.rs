use super::sql;
use crate::migrations;
use miden_crypto::{hash::rpo::RpoDigest, merkle::NodeIndex, StarkField};
use miden_node_proto::{
    block_header::BlockHeader as ProtobufBlockHeader,
    digest::Digest as ProtobufDigest,
    merkle::MerklePath,
    note::Note,
    responses::{AccountHashUpdate, NullifierUpdate},
};
use miden_objects::{crypto::merkle::SimpleSmt, notes::NOTE_LEAF_DEPTH, Felt, FieldElement};
use rusqlite::{vtab::array, Connection};

fn create_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    array::load_module(&conn).unwrap();
    migrations::MIGRATIONS.to_latest(&mut conn).unwrap();
    conn
}

#[test]
fn test_db_nullifiers() {
    let mut conn = create_db();

    // test querying empty table
    let nullifiers = sql::select_nullifiers(&mut conn).unwrap();
    assert!(nullifiers.is_empty());

    let nullifiers = sql::select_nullifiers_by_block_range(&mut conn, 0, u32::MAX, &[]).unwrap();
    assert!(nullifiers.is_empty());

    // test inserion
    let nullifier1 = num_to_rpo_digest(1 << 48);
    let block_number1 = 1;

    let transaction = conn.transaction().unwrap();
    sql::insert_nullifiers_for_block(&transaction, &[nullifier1], block_number1).unwrap();
    transaction.commit().unwrap();

    // test load
    let nullifiers = sql::select_nullifiers(&mut conn).unwrap();
    assert_eq!(nullifiers, vec![(nullifier1, block_number1)]);

    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        u32::MAX,
        &[sql::u64_to_prefix(nullifier1[0].as_int())],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierUpdate {
            nullifier: Some(nullifier1.into()),
            block_num: block_number1
        }]
    );

    // test additional element
    let nullifier2 = num_to_rpo_digest(2 << 48);
    let block_number2 = 2;

    let transaction = conn.transaction().unwrap();
    sql::insert_nullifiers_for_block(&transaction, &[nullifier2], block_number2).unwrap();
    transaction.commit().unwrap();

    let nullifiers = sql::select_nullifiers(&mut conn).unwrap();
    assert_eq!(nullifiers, vec![(nullifier1, block_number1), (nullifier2, block_number2)]);

    // only the nullifiers matching the prefix are included
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        u32::MAX,
        &[sql::u64_to_prefix(nullifier1[0].as_int())],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierUpdate {
            nullifier: Some(nullifier1.into()),
            block_num: block_number1
        }]
    );
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        u32::MAX,
        &[sql::u64_to_prefix(nullifier2[0].as_int())],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierUpdate {
            nullifier: Some(nullifier2.into()),
            block_num: block_number2
        }]
    );

    // Nullifiers created at block_end are included
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        1,
        &[
            sql::u64_to_prefix(nullifier1[0].as_int()),
            sql::u64_to_prefix(nullifier2[0].as_int()),
        ],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierUpdate {
            nullifier: Some(nullifier1.into()),
            block_num: block_number1
        }]
    );

    // Nullifiers created at block_start are not included
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        1,
        u32::MAX,
        &[
            sql::u64_to_prefix(nullifier1[0].as_int()),
            sql::u64_to_prefix(nullifier2[0].as_int()),
        ],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierUpdate {
            nullifier: Some(nullifier2.into()),
            block_num: block_number2
        }]
    );
}

#[test]
fn test_db_block_header() {
    let mut conn = create_db();

    // test querying empty table
    let block_number = 1;
    let res = sql::select_block_header_by_block_num(&mut conn, Some(block_number)).unwrap();
    assert!(res.is_none());

    let res = sql::select_block_header_by_block_num(&mut conn, None).unwrap();
    assert!(res.is_none());

    let res = sql::select_block_headers(&mut conn).unwrap();
    assert!(res.is_empty());

    let block_header = ProtobufBlockHeader {
        prev_hash: Some(num_to_protobuf_digest(1)),
        block_num: 2,
        chain_root: Some(num_to_protobuf_digest(3)),
        account_root: Some(num_to_protobuf_digest(4)),
        nullifier_root: Some(num_to_protobuf_digest(5)),
        note_root: Some(num_to_protobuf_digest(6)),
        batch_root: Some(num_to_protobuf_digest(7)),
        proof_hash: Some(num_to_protobuf_digest(8)),
        version: 9,
        timestamp: 10,
    };

    // test insertion
    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header).unwrap();
    transaction.commit().unwrap();

    // test fetch unknown block header
    let block_number = 1;
    let res = sql::select_block_header_by_block_num(&mut conn, Some(block_number)).unwrap();
    assert!(res.is_none());

    // test fetch block header by block number
    let res =
        sql::select_block_header_by_block_num(&mut conn, Some(block_header.block_num)).unwrap();
    assert_eq!(res.unwrap(), block_header);

    // test fetch latest block header
    let res = sql::select_block_header_by_block_num(&mut conn, None).unwrap();
    assert_eq!(res.unwrap(), block_header);

    let block_header2 = ProtobufBlockHeader {
        prev_hash: Some(num_to_protobuf_digest(11)),
        block_num: 12,
        chain_root: Some(num_to_protobuf_digest(13)),
        account_root: Some(num_to_protobuf_digest(14)),
        nullifier_root: Some(num_to_protobuf_digest(15)),
        note_root: Some(num_to_protobuf_digest(16)),
        batch_root: Some(num_to_protobuf_digest(17)),
        proof_hash: Some(num_to_protobuf_digest(18)),
        version: 19,
        timestamp: 20,
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header2).unwrap();
    transaction.commit().unwrap();

    let res = sql::select_block_header_by_block_num(&mut conn, None).unwrap();
    assert_eq!(res.unwrap(), block_header2);

    let res = sql::select_block_headers(&mut conn).unwrap();
    assert_eq!(res, [block_header, block_header2]);
}

#[test]
fn test_db_account() {
    let mut conn = create_db();

    // test empty table
    let account_ids = vec![0, 1, 2, 3, 4, 5];
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert!(res.is_empty());

    // test insertion
    let block_num = 1;
    let account_id = 0;
    let account_hash = num_to_protobuf_digest(0);

    let transaction = conn.transaction().unwrap();
    let row_count = sql::upsert_accounts_with_blocknum(
        &transaction,
        &[(account_id, account_hash.clone())],
        block_num,
    )
    .unwrap();
    transaction.commit().unwrap();

    assert_eq!(row_count, 1);

    // test successful query
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert_eq!(
        res,
        vec![AccountHashUpdate {
            account_id: Some(account_id.into()),
            account_hash: Some(account_hash.into()),
            block_num
        }]
    );

    // test query for update outside of the block range
    let res = sql::select_accounts_by_block_range(&mut conn, block_num + 1, u32::MAX, &account_ids)
        .unwrap();
    assert!(res.is_empty());

    // test query with unknown accounts
    let res = sql::select_accounts_by_block_range(&mut conn, block_num + 1, u32::MAX, &[6, 7, 8])
        .unwrap();
    assert!(res.is_empty());
}

#[test]
fn test_notes() {
    let mut conn = create_db();

    // test empty table
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[], &[], 0).unwrap();
    assert!(res.is_empty());

    let res =
        sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[1, 2, 3], &[], 0).unwrap();
    assert!(res.is_empty());

    // test insertion
    let block_num = 1;
    let note_index = 2u32;
    let tag = 5;
    let note_hash = num_to_rpo_digest(3);
    let values = [(note_index as u64, *note_hash)];
    let notes_db = SimpleSmt::with_leaves(NOTE_LEAF_DEPTH, values.iter().cloned()).unwrap();
    let idx = NodeIndex::new(NOTE_LEAF_DEPTH, note_index as u64).unwrap();
    let merkle_path = notes_db.get_path(idx).unwrap();

    let merkle_path: Vec<ProtobufDigest> =
        merkle_path.nodes().iter().map(|n| (*n).into()).collect();

    let note = Note {
        block_num,
        note_index,
        note_hash: Some(num_to_protobuf_digest(3)),
        sender: 4,
        tag,
        num_assets: 6,
        merkle_path: Some(MerklePath {
            siblings: merkle_path.clone(),
        }),
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_notes(&transaction, &[note.clone()]).unwrap();
    transaction.commit().unwrap();

    // test empty tags
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[], &[], 0).unwrap();
    assert!(res.is_empty());

    // test no updates
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num,
    )
    .unwrap();
    assert!(res.is_empty());

    // test match
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num - 1,
    )
    .unwrap();
    assert_eq!(res, vec![note.clone()]);

    // insertion second note with same tag, but on higher block
    let note2 = Note {
        block_num: note.block_num + 1,
        note_index: note.note_index,
        note_hash: Some(num_to_protobuf_digest(3)),
        sender: note.sender,
        tag: note.tag,
        num_assets: note.num_assets,
        merkle_path: Some(MerklePath {
            siblings: merkle_path,
        }),
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_notes(&transaction, &[note2.clone()]).unwrap();
    transaction.commit().unwrap();

    // only first note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num - 1,
    )
    .unwrap();
    assert_eq!(res, vec![note.clone()]);

    // only the second note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num,
    )
    .unwrap();
    assert_eq!(res, vec![note2.clone()]);
}

// UTILITIES
// -------------------------------------------------------------------------------------------
fn num_to_rpo_digest(n: u64) -> RpoDigest {
    RpoDigest::new([Felt::new(n), Felt::ZERO, Felt::ZERO, Felt::ZERO])
}

fn num_to_protobuf_digest(n: u64) -> ProtobufDigest {
    ProtobufDigest {
        d0: 0,
        d1: 0,
        d2: 0,
        d3: n,
    }
}
