use miden_lib::transaction::TransactionKernel;
use miden_node_proto::domain::accounts::AccountSummary;
use miden_objects::{
    accounts::{
        account_id::testing::{
            ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN, ACCOUNT_ID_NON_FUNGIBLE_FAUCET_ON_CHAIN,
            ACCOUNT_ID_OFF_CHAIN_SENDER, ACCOUNT_ID_REGULAR_ACCOUNT_IMMUTABLE_CODE_ON_CHAIN,
            ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN,
        },
        delta::AccountUpdateDetails,
        Account, AccountCode, AccountDelta, AccountId, AccountStorage, AccountStorageDelta,
        AccountVaultDelta,
    },
    assembly::{Assembler, ModuleAst},
    assets::{Asset, AssetVault, FungibleAsset, NonFungibleAsset, NonFungibleAssetDetails},
    block::{BlockAccountUpdate, BlockNoteIndex, BlockNoteTree},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    notes::{NoteId, NoteMetadata, NoteType, Nullifier},
    BlockHeader, Felt, FieldElement, Word, ONE, ZERO,
};
use rusqlite::{vtab::array, Connection};

use super::{sql, AccountInfo, NoteRecord, NullifierInfo};
use crate::db::migrations::apply_migrations;

fn create_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    array::load_module(&conn).unwrap();
    apply_migrations(&mut conn).unwrap();
    conn
}

fn create_block(conn: &mut Connection, block_num: u32) {
    let block_header = BlockHeader::new(
        1_u8.into(),
        num_to_rpo_digest(2),
        block_num,
        num_to_rpo_digest(4),
        num_to_rpo_digest(5),
        num_to_rpo_digest(6),
        num_to_rpo_digest(7),
        num_to_rpo_digest(8),
        num_to_rpo_digest(9),
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
fn test_sql_insert_transactions() {
    let mut conn = create_db();

    let count = insert_transactions(&mut conn);

    assert_eq!(count, 2, "Two elements must have been inserted");
}

#[test]
fn test_sql_select_transactions() {
    fn query_transactions(conn: &mut Connection) -> Vec<RpoDigest> {
        sql::select_transactions_by_accounts_and_block_range(conn, 1, 2, &[1]).unwrap()
    }

    let mut conn = create_db();

    let transactions = query_transactions(&mut conn);

    assert!(transactions.is_empty(), "No elements must be initially in the DB");

    let count = insert_transactions(&mut conn);

    assert_eq!(count, 2, "Two elements must have been inserted");
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
        let note = NoteRecord {
            block_num,
            note_index: BlockNoteIndex::new(0, i as usize),
            note_id: num_to_rpo_digest(i as u64),
            metadata: NoteMetadata::new(
                ACCOUNT_ID_OFF_CHAIN_SENDER.try_into().unwrap(),
                NoteType::Public,
                i.into(),
                Default::default(),
            )
            .unwrap(),
            details: Some(vec![1, 2, 3]),
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
        let account_id =
            ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN + (i << 32) + 0b1111100000;
        let account_hash = num_to_rpo_digest(i);
        state.push(AccountInfo {
            summary: AccountSummary {
                account_id: account_id.try_into().unwrap(),
                account_hash,
                block_num,
            },
            details: None,
        });

        let transaction = conn.transaction().unwrap();
        let res = sql::upsert_accounts(
            &transaction,
            &[BlockAccountUpdate::new(
                account_id.try_into().unwrap(),
                account_hash,
                AccountUpdateDetails::Private,
                vec![],
            )],
            block_num,
        );
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let accounts = sql::select_accounts(&mut conn).unwrap();
        assert_eq!(accounts, state);
    }
}

#[test]
fn test_sql_public_account_details() {
    let mut conn = create_db();

    let block_num = 1;
    create_block(&mut conn, block_num);

    let account_id =
        AccountId::try_from(ACCOUNT_ID_REGULAR_ACCOUNT_IMMUTABLE_CODE_ON_CHAIN).unwrap();
    let fungible_faucet_id = AccountId::try_from(ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN).unwrap();
    let non_fungible_faucet_id =
        AccountId::try_from(ACCOUNT_ID_NON_FUNGIBLE_FAUCET_ON_CHAIN).unwrap();

    let mut storage = AccountStorage::new(vec![], vec![]).unwrap();
    storage.set_item(1, num_to_word(1)).unwrap();
    storage.set_item(3, num_to_word(3)).unwrap();
    storage.set_item(5, num_to_word(5)).unwrap();

    let nft1 = Asset::NonFungible(
        NonFungibleAsset::new(
            &NonFungibleAssetDetails::new(non_fungible_faucet_id, vec![1, 2, 3]).unwrap(),
        )
        .unwrap(),
    );

    let mut account = Account::from_parts(
        account_id,
        AssetVault::new(&[
            Asset::Fungible(FungibleAsset::new(fungible_faucet_id, 150).unwrap()),
            nft1,
        ])
        .unwrap(),
        storage,
        mock_account_code(&TransactionKernel::assembler()),
        ZERO,
    );

    // test querying empty table
    let accounts_in_db = sql::select_accounts(&mut conn).unwrap();
    assert!(accounts_in_db.is_empty());

    let transaction = conn.transaction().unwrap();
    let inserted = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account_id,
            account.hash(),
            AccountUpdateDetails::New(account.clone()),
            vec![],
        )],
        block_num,
    )
    .unwrap();

    assert_eq!(inserted, 1, "One element must have been inserted");

    transaction.commit().unwrap();

    let mut accounts_in_db = sql::select_accounts(&mut conn).unwrap();

    assert_eq!(accounts_in_db.len(), 1, "One element must have been inserted");

    let account_read = accounts_in_db.pop().unwrap().details.unwrap();
    assert_eq!(account_read, account);

    let storage_delta = AccountStorageDelta {
        cleared_items: vec![3],
        updated_items: vec![(4, num_to_word(5)), (5, num_to_word(6))],
        updated_maps: vec![],
    };

    let nft2 = Asset::NonFungible(
        NonFungibleAsset::new(
            &NonFungibleAssetDetails::new(non_fungible_faucet_id, vec![4, 5, 6]).unwrap(),
        )
        .unwrap(),
    );

    let vault_delta = AccountVaultDelta {
        added_assets: vec![nft2],
        removed_assets: vec![nft1],
    };

    let delta = AccountDelta::new(storage_delta, vault_delta, Some(ONE)).unwrap();

    account.apply_delta(&delta).unwrap();

    let transaction = conn.transaction().unwrap();
    let inserted = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account_id,
            account.hash(),
            AccountUpdateDetails::Delta(delta.clone()),
            vec![],
        )],
        block_num,
    )
    .unwrap();

    assert_eq!(inserted, 1, "One element must have been inserted");

    transaction.commit().unwrap();

    let mut accounts_in_db = sql::select_accounts(&mut conn).unwrap();

    assert_eq!(accounts_in_db.len(), 1, "One element must have been inserted");

    let mut account_read = accounts_in_db.pop().unwrap().details.unwrap();

    assert_eq!(account_read.id(), account.id());
    assert_eq!(account_read.vault(), account.vault());
    assert_eq!(account_read.nonce(), account.nonce());

    // Cleared item was not serialized, check it and apply delta only with clear item second time:
    assert_eq!(account_read.storage().get_item(3), RpoDigest::default());

    let storage_delta = AccountStorageDelta {
        cleared_items: vec![3],
        updated_items: vec![],
        updated_maps: vec![],
    };
    account_read
        .apply_delta(
            &AccountDelta::new(storage_delta, AccountVaultDelta::default(), Some(Felt::new(2)))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(account_read.storage(), account.storage());
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
        1_u8.into(),
        num_to_rpo_digest(2),
        3,
        num_to_rpo_digest(4),
        num_to_rpo_digest(5),
        num_to_rpo_digest(6),
        num_to_rpo_digest(7),
        num_to_rpo_digest(8),
        num_to_rpo_digest(9),
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
        11_u8.into(),
        num_to_rpo_digest(12),
        13,
        num_to_rpo_digest(14),
        num_to_rpo_digest(15),
        num_to_rpo_digest(16),
        num_to_rpo_digest(17),
        num_to_rpo_digest(18),
        num_to_rpo_digest(19),
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
    let account_ids = vec![ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN, 1, 2, 3, 4, 5];
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert!(res.is_empty());

    // test insertion
    let account_id = ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN;
    let account_hash = num_to_rpo_digest(0);

    let transaction = conn.transaction().unwrap();
    let row_count = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account_id.try_into().unwrap(),
            account_hash,
            AccountUpdateDetails::Private,
            vec![],
        )],
        block_num,
    )
    .unwrap();
    transaction.commit().unwrap();

    assert_eq!(row_count, 1);

    // test successful query
    let res = sql::select_accounts_by_block_range(&mut conn, 0, u32::MAX, &account_ids).unwrap();
    assert_eq!(
        res,
        vec![AccountSummary {
            account_id: account_id.try_into().unwrap(),
            account_hash,
            block_num,
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
    let note_index = BlockNoteIndex::new(0, 2);
    let note_id = num_to_rpo_digest(3);
    let tag = 5u32;
    let sender = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));
    let note_metadata = NoteMetadata::new(sender, NoteType::Public, tag.into(), ZERO).unwrap();

    let values = [(note_index, note_id, note_metadata)];
    let notes_db = BlockNoteTree::with_entries(values.iter().cloned()).unwrap();
    let details = Some(vec![1, 2, 3]);
    let merkle_path = notes_db.get_note_path(note_index).unwrap();

    let note = NoteRecord {
        block_num: block_num_1,
        note_index,
        note_id,
        metadata: NoteMetadata::new(sender, NoteType::Public, tag.into(), Default::default())
            .unwrap(),
        details,
        merkle_path: merkle_path.clone(),
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_notes(&transaction, &[note.clone()]).unwrap();
    transaction.commit().unwrap();

    // test empty tags
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[], &[], 0).unwrap();
    assert!(res.is_empty());

    // test no updates
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[tag], &[], block_num_1)
        .unwrap();
    assert!(res.is_empty());

    // test match
    let res =
        sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[tag], &[], block_num_1 - 1)
            .unwrap();
    assert_eq!(res, vec![note.clone()]);

    let block_num_2 = note.block_num + 1;
    create_block(&mut conn, block_num_2);

    // insertion second note with same tag, but on higher block
    let note2 = NoteRecord {
        block_num: block_num_2,
        note_index: note.note_index,
        note_id: num_to_rpo_digest(3),
        metadata: note.metadata,
        details: None,
        merkle_path,
    };

    let transaction = conn.transaction().unwrap();
    sql::insert_notes(&transaction, &[note2.clone()]).unwrap();
    transaction.commit().unwrap();

    // only first note is returned
    let res =
        sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[tag], &[], block_num_1 - 1)
            .unwrap();
    assert_eq!(res, vec![note.clone()]);

    // only the second note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(&mut conn, &[tag], &[], block_num_1)
        .unwrap();
    assert_eq!(res, vec![note2.clone()]);

    // test query notes by id
    let notes = vec![note, note2];
    let note_ids: Vec<RpoDigest> = notes.clone().iter().map(|note| note.note_id).collect();
    let note_ids: Vec<NoteId> = note_ids.into_iter().map(From::from).collect();

    let res = sql::select_notes_by_id(&mut conn, &note_ids).unwrap();
    assert_eq!(res, notes);

    // test notes have correct details
    let note_0 = res[0].clone();
    let note_1 = res[1].clone();
    assert_eq!(note_0.details, Some(vec![1, 2, 3]));
    assert_eq!(note_1.details, None)
}

// UTILITIES
// -------------------------------------------------------------------------------------------
fn num_to_rpo_digest(n: u64) -> RpoDigest {
    RpoDigest::new(num_to_word(n))
}

fn num_to_word(n: u64) -> Word {
    [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(n)]
}

fn num_to_nullifier(n: u64) -> Nullifier {
    Nullifier::from(num_to_rpo_digest(n))
}

fn mock_block_account_update(account_id: AccountId, num: u64) -> BlockAccountUpdate {
    BlockAccountUpdate::new(
        account_id,
        num_to_rpo_digest(num),
        AccountUpdateDetails::Private,
        vec![num_to_rpo_digest(num + 1000).into(), num_to_rpo_digest(num + 1001).into()],
    )
}

fn insert_transactions(conn: &mut Connection) -> usize {
    let block_num = 1;
    create_block(conn, block_num);

    let transaction = conn.transaction().unwrap();
    let count = sql::insert_transactions(
        &transaction,
        block_num,
        &[mock_block_account_update(AccountId::new_unchecked(Felt::ONE), 1)],
    )
    .unwrap();
    transaction.commit().unwrap();

    count
}

pub fn mock_account_code(assembler: &Assembler) -> AccountCode {
    let account_code = "\
            export.account_procedure_1
                push.1.2
                add
            end
            ";
    let account_module_ast = ModuleAst::parse(account_code).unwrap();
    AccountCode::new(account_module_ast, assembler).unwrap()
}
