use miden_objects::{
    accounts::{AccountId, ACCOUNT_ID_OFF_CHAIN_SENDER},
    block::BlockNoteTree,
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    notes::{NoteMetadata, NoteType, Nullifier},
    BlockHeader, Felt, FieldElement, ZERO,
};
use rusqlite::{vtab::array, Connection};

use super::{sql, AccountInfo, Note, NoteCreated, NullifierInfo};
use crate::db::migrations;

fn create_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    array::load_module(&conn).unwrap();
    migrations::MIGRATIONS.to_latest(&mut conn).unwrap();
    conn
}

fn create_block(
    conn: &mut Connection,
    block_num: u32,
) {
    let block_header = BlockHeader::new(
        num_to_rpo_digest(1),
        block_num,
        num_to_rpo_digest(3),
        num_to_rpo_digest(4),
        num_to_rpo_digest(5),
        num_to_rpo_digest(6),
        num_to_rpo_digest(7),
        num_to_rpo_digest(8),
        9_u8.into(),
        10_u8.into(),
    );

    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header).unwrap();
    transaction.commit().unwrap();
}

#[test]
fn test_sql_insert_nullifiers_for_block() {
    let mut conn = create_db();

    let nullifiers = [num_to_nullifier(1 << 48)];

    let block_num = 1;
    create_block(&mut conn, block_num);

    // Insert a new nullifier succeeds
    {
        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &nullifiers, block_num);
        assert_eq!(res.unwrap(), nullifiers.len(), "There should be one entry");
        transaction.commit().unwrap();
    }

    // Inserting the nullifier twice is an error
    {
        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &nullifiers, block_num);
        assert!(res.is_err(), "Inserting the same nullifier twice is an error");
    }

    // even if the block number is different
    {
        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &nullifiers, block_num + 1);
        transaction.commit().unwrap();
        assert!(
            res.is_err(),
            "Inserting the same nullifier twice is an error, even if with a different block number"
        );
    }

    // test inserting multiple nullifiers
    {
        let nullifiers: Vec<_> = (0..10).map(num_to_nullifier).collect();
        let block_num = 1;
        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &nullifiers, block_num);
        transaction.commit().unwrap();
        assert_eq!(res.unwrap(), nullifiers.len(), "There should be 10 entries");
    }
}

#[test]
fn test_sql_select_nullifiers() {
    let mut conn = create_db();

    let block_num = 1;
    create_block(&mut conn, block_num);

    // test querying empty table
    let nullifiers = sql::select_nullifiers(&mut conn).unwrap();
    assert!(nullifiers.is_empty());

    // test multiple entries
    let mut state = vec![];
    for i in 0..10 {
        let nullifier = num_to_nullifier(i);
        state.push((nullifier, block_num));

        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &[nullifier], block_num);
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let nullifiers = sql::select_nullifiers(&mut conn).unwrap();
        assert_eq!(nullifiers, state);
    }
}

#[test]
fn test_sql_select_notes() {
    let mut conn = create_db();

    let block_num = 1;
    create_block(&mut conn, block_num);

    // test querying empty table
    let notes = sql::select_notes(&mut conn).unwrap();
    assert!(notes.is_empty());

    // test multiple entries
    let mut state = vec![];
    for i in 0..10 {
        let note = Note {
            block_num,
            note_created: NoteCreated {
                batch_index: 0,
                note_index: i,
                note_id: num_to_rpo_digest(i as u64),
                sender: i as u64,
                tag: i as u64,
            },
            merkle_path: MerklePath::new(vec![]),
        };
        state.push(note.clone());

        let transaction = conn.transaction().unwrap();
        let res = sql::insert_notes(&transaction, &[note]);
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let notes = sql::select_notes(&mut conn).unwrap();
        assert_eq!(notes, state);
    }
}

#[test]
fn test_sql_select_accounts() {
    let mut conn = create_db();

    let block_num = 1;
    create_block(&mut conn, block_num);

    // test querying empty table
    let accounts = sql::select_accounts(&mut conn).unwrap();
    assert!(accounts.is_empty());

    // test multiple entries
    let mut state = vec![];
    for i in 0..10 {
        let account_id = i;
        let account_hash = num_to_rpo_digest(i);
        state.push(AccountInfo {
            account_id,
            account_hash,
            block_num,
        });

        let transaction = conn.transaction().unwrap();
        let res = sql::upsert_accounts(&transaction, &[(account_id, account_hash)], block_num);
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let accounts = sql::select_accounts(&mut conn).unwrap();
        assert_eq!(accounts, state);
    }
}

#[test]
fn test_sql_select_nullifiers_by_block_range() {
    let mut conn = create_db();

    // test empty table
    let nullifiers = sql::select_nullifiers_by_block_range(&mut conn, 0, u32::MAX, &[]).unwrap();
    assert!(nullifiers.is_empty());

    // test single item
    let nullifier1 = num_to_nullifier(1 << 48);
    let block_number1 = 1;
    create_block(&mut conn, block_number1);

    let transaction = conn.transaction().unwrap();
    sql::insert_nullifiers_for_block(&transaction, &[nullifier1], block_number1).unwrap();
    transaction.commit().unwrap();

    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        u32::MAX,
        &[sql::get_nullifier_prefix(&nullifier1)],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier1,
            block_num: block_number1
        }]
    );

    // test two elements
    let nullifier2 = num_to_nullifier(2 << 48);
    let block_number2 = 2;
    create_block(&mut conn, block_number2);

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
        &[sql::get_nullifier_prefix(&nullifier1)],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier1,
            block_num: block_number1
        }]
    );
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        u32::MAX,
        &[sql::get_nullifier_prefix(&nullifier2)],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier2,
            block_num: block_number2
        }]
    );

    // Nullifiers created at block_end are included
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        0,
        1,
        &[sql::get_nullifier_prefix(&nullifier1), sql::get_nullifier_prefix(&nullifier2)],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier1,
            block_num: block_number1
        }]
    );

    // Nullifiers created at block_start are not included
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        1,
        u32::MAX,
        &[sql::get_nullifier_prefix(&nullifier1), sql::get_nullifier_prefix(&nullifier2)],
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier2,
            block_num: block_number2
        }]
    );

    // When block start and end are the same, no nullifiers should be returned. This case happens
    // when the client requests a sync update, and it is already tracking the chain tip.
    let nullifiers = sql::select_nullifiers_by_block_range(
        &mut conn,
        2,
        2,
        &[sql::get_nullifier_prefix(&nullifier1), sql::get_nullifier_prefix(&nullifier2)],
    )
    .unwrap();
    assert!(nullifiers.is_empty());
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

    let block_header = BlockHeader::new(
        num_to_rpo_digest(1),
        2,
        num_to_rpo_digest(3),
        num_to_rpo_digest(4),
        num_to_rpo_digest(5),
        num_to_rpo_digest(6),
        num_to_rpo_digest(7),
        num_to_rpo_digest(8),
        9_u8.into(),
        10_u8.into(),
    );

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
        sql::select_block_header_by_block_num(&mut conn, Some(block_header.block_num())).unwrap();
    assert_eq!(res.unwrap(), block_header);

    // test fetch latest block header
    let res = sql::select_block_header_by_block_num(&mut conn, None).unwrap();
    assert_eq!(res.unwrap(), block_header);

    let block_header2 = BlockHeader::new(
        num_to_rpo_digest(11),
        12,
        num_to_rpo_digest(13),
        num_to_rpo_digest(14),
        num_to_rpo_digest(15),
        num_to_rpo_digest(16),
        num_to_rpo_digest(17),
        num_to_rpo_digest(18),
        19_u8.into(),
        20_u8.into(),
    );

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

    let block_num = 1;
    create_block(&mut conn, block_num);

    // test empty table
    let account_ids = vec![0, 1, 2, 3, 4, 5];
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert!(res.is_empty());

    // test insertion
    let account_id = 0;
    let account_hash = num_to_rpo_digest(0);

    let transaction = conn.transaction().unwrap();
    let row_count =
        sql::upsert_accounts(&transaction, &[(account_id, account_hash)], block_num).unwrap();
    transaction.commit().unwrap();

    assert_eq!(row_count, 1);

    // test successful query
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert_eq!(
        res,
        vec![AccountInfo {
            account_id,
            account_hash,
            block_num
        }]
    );

    // test query for update outside the block range
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

    let block_num_1 = 1;
    create_block(&mut conn, block_num_1);

    // test empty table
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[], &[], 0).unwrap();
    assert!(res.is_empty());

    let res =
        sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[1, 2, 3], &[], 0).unwrap();
    assert!(res.is_empty());

    // test insertion
    let batch_index = 0u32;
    let note_index = 2u32;
    let note_id = num_to_rpo_digest(3);
    let tag = 5u64;
    // Precomputed seed for regular off-chain account for zeroed initial seed:
    let sender = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));
    let note_metadata =
        NoteMetadata::new(sender, NoteType::OffChain, (tag as u32).into(), ZERO).unwrap();

    let values = [(batch_index as usize, note_index as usize, (note_id, note_metadata))];
    let notes_db = BlockNoteTree::with_entries(values.iter().cloned()).unwrap();
    let merkle_path = notes_db.get_note_path(batch_index as usize, note_index as usize).unwrap();

    let note = Note {
        block_num: block_num_1,
        note_created: NoteCreated {
            batch_index,
            note_index,
            note_id,
            sender: sender.into(),
            tag,
        },
        merkle_path: merkle_path.clone(),
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
        block_num_1,
    )
    .unwrap();
    assert!(res.is_empty());

    // test match
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num_1 - 1,
    )
    .unwrap();
    assert_eq!(res, vec![note.clone()]);

    let block_num_2 = note.block_num + 1;
    create_block(&mut conn, block_num_2);

    // insertion second note with same tag, but on higher block
    let note2 = Note {
        block_num: block_num_2,
        note_created: NoteCreated {
            batch_index: note.note_created.batch_index,
            note_index: note.note_created.note_index,
            note_id: num_to_rpo_digest(3),
            sender: note.note_created.sender,
            tag: note.note_created.tag,
        },
        merkle_path,
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_notes(&transaction, &[note2.clone()]).unwrap();
    transaction.commit().unwrap();

    // only first note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num_1 - 1,
    )
    .unwrap();
    assert_eq!(res, vec![note.clone()]);

    // only the second note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &mut conn,
        &[(tag >> 48) as u32],
        &[],
        block_num_1,
    )
    .unwrap();
    assert_eq!(res, vec![note2.clone()]);
}

// UTILITIES
// -------------------------------------------------------------------------------------------
fn num_to_rpo_digest(n: u64) -> RpoDigest {
    RpoDigest::new([Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(n)])
}

fn num_to_nullifier(n: u64) -> Nullifier {
    Nullifier::from(num_to_rpo_digest(n))
}
