#![allow(clippy::similar_names, reason = "naming dummy test values is hard")]
#![allow(clippy::too_many_lines, reason = "test code can be long")]

use std::num::NonZeroUsize;

use miden_lib::transaction::TransactionKernel;
use miden_node_proto::domain::account::AccountSummary;
use miden_objects::{
    Felt, FieldElement, Word, ZERO,
    account::{
        Account, AccountBuilder, AccountComponent, AccountDelta, AccountId, AccountIdVersion,
        AccountStorageDelta, AccountStorageMode, AccountType, AccountVaultDelta, StorageSlot,
        delta::AccountUpdateDetails,
    },
    asset::{Asset, FungibleAsset, NonFungibleAsset, NonFungibleAssetDetails},
    block::{BlockAccountUpdate, BlockHeader, BlockNoteIndex, BlockNoteTree, BlockNumber},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    note::{
        NoteExecutionHint, NoteExecutionMode, NoteId, NoteMetadata, NoteTag, NoteType, Nullifier,
    },
    testing::account_id::{
        ACCOUNT_ID_PRIVATE_SENDER, ACCOUNT_ID_PUBLIC_FUNGIBLE_FAUCET,
        ACCOUNT_ID_PUBLIC_NON_FUNGIBLE_FAUCET, ACCOUNT_ID_REGULAR_PRIVATE_ACCOUNT_UPDATABLE_CODE,
    },
};

use super::{AccountInfo, NoteRecord, NullifierInfo, sql};
use crate::db::{
    TransactionSummary, connection::Connection, migrations::apply_migrations, sql::Page,
};

fn create_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    apply_migrations(&mut conn).unwrap();
    conn
}

fn create_block(conn: &mut Connection, block_num: BlockNumber) {
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
        num_to_rpo_digest(10),
        11_u8.into(),
    );

    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header).unwrap();
    transaction.commit().unwrap();
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_insert_nullifiers_for_block() {
    let mut conn = create_db();

    let nullifiers = [num_to_nullifier(1 << 48)];

    let block_num = 1.into();
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
        let block_num = 1.into();
        let transaction = conn.transaction().unwrap();
        let res = sql::insert_nullifiers_for_block(&transaction, &nullifiers, block_num);
        transaction.commit().unwrap();
        assert_eq!(res.unwrap(), nullifiers.len(), "There should be 10 entries");
    }
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_insert_transactions() {
    let mut conn = create_db();

    let count = insert_transactions(&mut conn);

    assert_eq!(count, 2, "Two elements must have been inserted");
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_select_transactions() {
    fn query_transactions(conn: &mut Connection) -> Vec<TransactionSummary> {
        sql::select_transactions_by_accounts_and_block_range(
            &conn.transaction().unwrap(),
            0.into(),
            2.into(),
            &[AccountId::try_from(ACCOUNT_ID_PRIVATE_SENDER).unwrap()],
        )
        .unwrap()
    }

    let mut conn = create_db();

    let transactions = query_transactions(&mut conn);

    assert!(transactions.is_empty(), "No elements must be initially in the DB");

    let count = insert_transactions(&mut conn);

    assert_eq!(count, 2, "Two elements must have been inserted");

    let transactions = query_transactions(&mut conn);

    assert_eq!(transactions.len(), 2, "Two elements must be in the DB");
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_select_nullifiers() {
    let mut conn = create_db();

    let block_num = 1.into();
    create_block(&mut conn, block_num);

    // test querying empty table
    let nullifiers = sql::select_all_nullifiers(&conn.transaction().unwrap()).unwrap();
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
        let nullifiers = sql::select_all_nullifiers(&conn.transaction().unwrap()).unwrap();
        assert_eq!(nullifiers, state);
    }
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_select_notes() {
    let mut conn = create_db();

    let block_num = BlockNumber::from(1);
    create_block(&mut conn, block_num);

    // test querying empty table
    let notes = sql::select_all_notes(&conn.transaction().unwrap()).unwrap();
    assert!(notes.is_empty());

    let account_id = AccountId::try_from(ACCOUNT_ID_PRIVATE_SENDER).unwrap();

    let transaction = conn.transaction().unwrap();

    sql::upsert_accounts(&transaction, &[mock_block_account_update(account_id, 0)], block_num)
        .unwrap();

    transaction.commit().unwrap();

    // test multiple entries
    let mut state = vec![];
    for i in 0..10 {
        let note = NoteRecord {
            block_num,
            note_index: BlockNoteIndex::new(0, i as usize).unwrap(),
            note_id: num_to_rpo_digest(u64::from(i)),
            metadata: NoteMetadata::new(
                account_id,
                NoteType::Public,
                i.into(),
                NoteExecutionHint::none(),
                Felt::default(),
            )
            .unwrap(),
            details: Some(vec![1, 2, 3]),
            merkle_path: MerklePath::new(vec![]),
        };
        state.push(note.clone());

        let transaction = conn.transaction().unwrap();
        let res = sql::insert_notes(&transaction, &[(note, None)]);
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let notes = sql::select_all_notes(&conn.transaction().unwrap()).unwrap();
        assert_eq!(notes, state);
    }
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_select_notes_different_execution_hints() {
    let mut conn = create_db();

    let block_num = 1.into();
    create_block(&mut conn, block_num);

    // test querying empty table
    let notes = sql::select_all_notes(&conn.transaction().unwrap()).unwrap();
    assert!(notes.is_empty());

    let sender = AccountId::try_from(ACCOUNT_ID_PRIVATE_SENDER).unwrap();

    let transaction = conn.transaction().unwrap();

    sql::upsert_accounts(&transaction, &[mock_block_account_update(sender, 0)], block_num).unwrap();

    transaction.commit().unwrap();

    // test multiple entries
    let mut state = vec![];

    let note_none = NoteRecord {
        block_num,
        note_index: BlockNoteIndex::new(0, 0).unwrap(),
        note_id: num_to_rpo_digest(0),
        metadata: NoteMetadata::new(
            sender,
            NoteType::Public,
            0.into(),
            NoteExecutionHint::none(),
            Felt::default(),
        )
        .unwrap(),
        details: Some(vec![1, 2, 3]),
        merkle_path: MerklePath::new(vec![]),
    };
    state.push(note_none.clone());

    let transaction = conn.transaction().unwrap();
    let res = sql::insert_notes(&transaction, &[(note_none, None)]);
    assert_eq!(res.unwrap(), 1, "One element must have been inserted");
    transaction.commit().unwrap();
    let note =
        &sql::select_notes_by_id(&conn.transaction().unwrap(), &[num_to_rpo_digest(0).into()])
            .unwrap()[0];
    assert_eq!(note.metadata.execution_hint(), NoteExecutionHint::none());

    let note_always = NoteRecord {
        block_num,
        note_index: BlockNoteIndex::new(0, 1).unwrap(),
        note_id: num_to_rpo_digest(1),
        metadata: NoteMetadata::new(
            sender,
            NoteType::Public,
            1.into(),
            NoteExecutionHint::always(),
            Felt::default(),
        )
        .unwrap(),
        details: Some(vec![1, 2, 3]),
        merkle_path: MerklePath::new(vec![]),
    };
    state.push(note_always.clone());

    let transaction = conn.transaction().unwrap();
    let res = sql::insert_notes(&transaction, &[(note_always, None)]);
    assert_eq!(res.unwrap(), 1, "One element must have been inserted");
    transaction.commit().unwrap();
    let note =
        &sql::select_notes_by_id(&conn.transaction().unwrap(), &[num_to_rpo_digest(1).into()])
            .unwrap()[0];
    assert_eq!(note.metadata.execution_hint(), NoteExecutionHint::always());

    let note_after_block = NoteRecord {
        block_num,
        note_index: BlockNoteIndex::new(0, 2).unwrap(),
        note_id: num_to_rpo_digest(2),
        metadata: NoteMetadata::new(
            sender,
            NoteType::Public,
            2.into(),
            NoteExecutionHint::after_block(12.into()).unwrap(),
            Felt::default(),
        )
        .unwrap(),
        details: Some(vec![1, 2, 3]),
        merkle_path: MerklePath::new(vec![]),
    };
    state.push(note_after_block.clone());

    let transaction = conn.transaction().unwrap();
    let res = sql::insert_notes(&transaction, &[(note_after_block, None)]);
    assert_eq!(res.unwrap(), 1, "One element must have been inserted");
    transaction.commit().unwrap();
    let note =
        &sql::select_notes_by_id(&conn.transaction().unwrap(), &[num_to_rpo_digest(2).into()])
            .unwrap()[0];
    assert_eq!(
        note.metadata.execution_hint(),
        NoteExecutionHint::after_block(12.into()).unwrap()
    );
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_unconsumed_network_notes() {
    // Number of notes to generate.
    const N: u64 = 32;

    let mut conn = create_db();

    let block_num = BlockNumber::from(1);
    // An arbitrary public account (network note tag requires public account).
    create_block(&mut conn, block_num);

    let transaction = conn.transaction().unwrap();

    let account = mock_account_code_and_storage(
        AccountType::RegularAccountUpdatableCode,
        AccountStorageMode::Public,
        [],
    );
    let account_id = account.id();
    sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account_id,
            account.commitment(),
            AccountUpdateDetails::New(account),
            vec![],
        )],
        block_num,
    )
    .unwrap();

    transaction.commit().unwrap();

    // Create some notes, of which half are network notes.
    let notes = (0..N)
        .map(|i| {
            let is_network = i % 2 == 0;
            let execution_mode = if is_network {
                NoteExecutionMode::Network
            } else {
                NoteExecutionMode::Local
            };
            let note = NoteRecord {
                block_num,
                note_index: BlockNoteIndex::new(0, i as usize).unwrap(),
                note_id: num_to_rpo_digest(i),
                metadata: NoteMetadata::new(
                    account_id,
                    NoteType::Public,
                    NoteTag::from_account_id(account_id, execution_mode).unwrap(),
                    NoteExecutionHint::none(),
                    Felt::default(),
                )
                .unwrap(),
                details: is_network.then_some(vec![1, 2, 3]),
                merkle_path: MerklePath::new(vec![]),
            };

            (note, is_network.then_some(num_to_nullifier(i)))
        })
        .collect::<Vec<_>>();

    // Copy out all network notes to assert against. These will be in chronological order already.
    let network_notes = notes
        .iter()
        .filter_map(|(note, nullifier)| nullifier.is_some().then_some(note.clone()))
        .collect::<Vec<_>>();

    // Insert the set of notes.
    let db_tx = conn.transaction().unwrap();
    sql::insert_notes(&db_tx, &notes).unwrap();

    // Fetch all network notes by setting a limit larger than the amount available.
    let (result, _) = sql::unconsumed_network_notes(
        &db_tx,
        Page {
            token: None,
            size: NonZeroUsize::new(N as usize * 10).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(result, network_notes);

    // Check pagination works as expected.
    let limit = 5;
    let mut page = Page {
        token: None,
        size: NonZeroUsize::new(limit).unwrap(),
    };
    network_notes.chunks(limit).for_each(|expected| {
        let (result, new_page) = sql::unconsumed_network_notes(&db_tx, page).unwrap();
        page = new_page;
        assert_eq!(result, expected);
    });
    assert!(page.token.is_none());

    // Consume every third network note and ensure these are now excluded from the results.
    let consumed = notes
        .iter()
        .filter_map(|(_, nullifier)| *nullifier)
        .step_by(3)
        .collect::<Vec<_>>();
    sql::insert_nullifiers_for_block(&db_tx, &consumed, block_num).unwrap();

    let expected = network_notes
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 3 != 0)
        .map(|(_, note)| note.clone())
        .collect::<Vec<_>>();
    let page = Page {
        token: None,
        size: NonZeroUsize::new(N as usize * 10).unwrap(),
    };
    let (result, _) = sql::unconsumed_network_notes(&db_tx, page).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_select_accounts() {
    let mut conn = create_db();

    let block_num = 1.into();
    create_block(&mut conn, block_num);

    // test querying empty table
    let accounts = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();
    assert!(accounts.is_empty());
    // test multiple entries
    let mut state = vec![];
    for i in 0..10u8 {
        let account_id = AccountId::dummy(
            [i; 15],
            AccountIdVersion::Version0,
            AccountType::RegularAccountImmutableCode,
            AccountStorageMode::Private,
        );
        let account_commitment = num_to_rpo_digest(u64::from(i));
        state.push(AccountInfo {
            summary: AccountSummary {
                account_id,
                account_commitment,
                block_num,
            },
            details: None,
        });

        let transaction = conn.transaction().unwrap();
        let res = sql::upsert_accounts(
            &transaction,
            &[BlockAccountUpdate::new(
                account_id,
                account_commitment,
                AccountUpdateDetails::Private,
                vec![],
            )],
            block_num,
        );
        assert_eq!(res.unwrap(), 1, "One element must have been inserted");
        transaction.commit().unwrap();
        let accounts = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();
        assert_eq!(accounts, state);
    }
}

#[test]
#[miden_node_test_macro::enable_logging]
fn sql_public_account_details() {
    let mut conn = create_db();

    create_block(&mut conn, 1.into());

    let fungible_faucet_id = AccountId::try_from(ACCOUNT_ID_PUBLIC_FUNGIBLE_FAUCET).unwrap();
    let non_fungible_faucet_id =
        AccountId::try_from(ACCOUNT_ID_PUBLIC_NON_FUNGIBLE_FAUCET).unwrap();

    let nft1 = Asset::NonFungible(
        NonFungibleAsset::new(
            &NonFungibleAssetDetails::new(non_fungible_faucet_id.prefix(), vec![1, 2, 3]).unwrap(),
        )
        .unwrap(),
    );

    let mut account = mock_account_code_and_storage(
        AccountType::RegularAccountImmutableCode,
        AccountStorageMode::Public,
        [Asset::Fungible(FungibleAsset::new(fungible_faucet_id, 150).unwrap()), nft1],
    );

    // test querying empty table
    let accounts_in_db = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();
    assert!(accounts_in_db.is_empty());

    let transaction = conn.transaction().unwrap();
    let inserted = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account.id(),
            account.commitment(),
            AccountUpdateDetails::New(account.clone()),
            vec![],
        )],
        1.into(),
    )
    .unwrap();

    assert_eq!(inserted, 1, "One element must have been inserted");

    transaction.commit().unwrap();

    let mut accounts_in_db = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();

    assert_eq!(accounts_in_db.len(), 1, "One element must have been inserted");

    let account_read = accounts_in_db.pop().unwrap().details.unwrap();
    assert_eq!(account_read, account);

    create_block(&mut conn, 2.into());

    let read_delta =
        sql::select_account_delta(&conn.transaction().unwrap(), account.id(), 1.into(), 2.into())
            .unwrap();

    assert_eq!(read_delta, None);

    let storage_delta =
        AccountStorageDelta::from_iters([3], [(4, num_to_word(5)), (5, num_to_word(6))], []);

    let nft2 = Asset::NonFungible(
        NonFungibleAsset::new(
            &NonFungibleAssetDetails::new(non_fungible_faucet_id.prefix(), vec![4, 5, 6]).unwrap(),
        )
        .unwrap(),
    );

    let vault_delta = AccountVaultDelta::from_iters([nft2], [nft1]);

    let mut delta2 = AccountDelta::new(storage_delta, vault_delta, Some(Felt::new(2))).unwrap();

    account.apply_delta(&delta2).unwrap();

    let transaction = conn.transaction().unwrap();
    let inserted = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account.id(),
            account.commitment(),
            AccountUpdateDetails::Delta(delta2.clone()),
            vec![],
        )],
        2.into(),
    )
    .unwrap();

    assert_eq!(inserted, 1, "One element must have been inserted");

    transaction.commit().unwrap();

    let mut accounts_in_db = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();

    assert_eq!(accounts_in_db.len(), 1, "One element must have been inserted");

    let account_read = accounts_in_db.pop().unwrap().details.unwrap();

    assert_eq!(account_read.id(), account.id());
    assert_eq!(account_read.vault(), account.vault());
    assert_eq!(account_read.nonce(), account.nonce());
    assert_eq!(account_read.storage(), account.storage());

    let read_delta =
        sql::select_account_delta(&conn.transaction().unwrap(), account.id(), 1.into(), 2.into())
            .unwrap();
    assert_eq!(read_delta.as_ref(), Some(&delta2));

    create_block(&mut conn, 3.into());

    let storage_delta3 = AccountStorageDelta::from_iters([5], [], []);

    let delta3 = AccountDelta::new(
        storage_delta3,
        AccountVaultDelta::from_iters([nft1], []),
        Some(Felt::new(3)),
    )
    .unwrap();

    account.apply_delta(&delta3).unwrap();

    let transaction = conn.transaction().unwrap();
    let inserted = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account.id(),
            account.commitment(),
            AccountUpdateDetails::Delta(delta3.clone()),
            vec![],
        )],
        3.into(),
    )
    .unwrap();

    assert_eq!(inserted, 1, "One element must have been inserted");

    transaction.commit().unwrap();

    let mut accounts_in_db = sql::select_all_accounts(&conn.transaction().unwrap()).unwrap();

    assert_eq!(accounts_in_db.len(), 1, "One element must have been inserted");

    let account_read = accounts_in_db.pop().unwrap().details.unwrap();

    assert_eq!(account_read.id(), account.id());
    assert_eq!(account_read.vault(), account.vault());
    assert_eq!(account_read.nonce(), account.nonce());

    let read_delta =
        sql::select_account_delta(&conn.transaction().unwrap(), account.id(), 1.into(), 3.into())
            .unwrap();

    delta2.merge(delta3).unwrap();

    assert_eq!(read_delta, Some(delta2));
}

#[test]
#[miden_node_test_macro::enable_logging]
fn select_nullifiers_by_prefix() {
    const PREFIX_LEN: u32 = 16;
    let mut conn = create_db();
    // test empty table
    let block_number0 = 0.into();
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[],
        block_number0,
    )
    .unwrap();
    assert!(nullifiers.is_empty());

    // test single item
    let nullifier1 = num_to_nullifier(1 << 48);
    let block_number1 = 1.into();
    create_block(&mut conn, block_number1);

    let transaction = conn.transaction().unwrap();
    sql::insert_nullifiers_for_block(&transaction, &[nullifier1], block_number1).unwrap();
    transaction.commit().unwrap();

    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[sql::utils::get_nullifier_prefix(&nullifier1)],
        block_number0,
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
    let block_number2 = 2.into();
    create_block(&mut conn, block_number2);

    let transaction = conn.transaction().unwrap();
    sql::insert_nullifiers_for_block(&transaction, &[nullifier2], block_number2).unwrap();
    transaction.commit().unwrap();

    let nullifiers = sql::select_all_nullifiers(&conn.transaction().unwrap()).unwrap();
    assert_eq!(nullifiers, vec![(nullifier1, block_number1), (nullifier2, block_number2)]);

    // only the nullifiers matching the prefix are included
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[sql::utils::get_nullifier_prefix(&nullifier1)],
        block_number0,
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier1,
            block_num: block_number1
        }]
    );
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[sql::utils::get_nullifier_prefix(&nullifier2)],
        block_number0,
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier2,
            block_num: block_number2
        }]
    );

    // All matching nullifiers are included
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[
            sql::utils::get_nullifier_prefix(&nullifier1),
            sql::utils::get_nullifier_prefix(&nullifier2),
        ],
        block_number0,
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![
            NullifierInfo {
                nullifier: nullifier1,
                block_num: block_number1
            },
            NullifierInfo {
                nullifier: nullifier2,
                block_num: block_number2
            }
        ]
    );

    // If a non-matching prefix is provided, no nullifiers are returned
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[sql::utils::get_nullifier_prefix(&num_to_nullifier(3 << 48))],
        block_number0,
    )
    .unwrap();
    assert!(nullifiers.is_empty());

    // If a block number is provided, only matching nullifiers created at or after that block are
    // returned
    let nullifiers = sql::select_nullifiers_by_prefix(
        &conn.transaction().unwrap(),
        PREFIX_LEN,
        &[
            sql::utils::get_nullifier_prefix(&nullifier1),
            sql::utils::get_nullifier_prefix(&nullifier2),
        ],
        block_number2,
    )
    .unwrap();
    assert_eq!(
        nullifiers,
        vec![NullifierInfo {
            nullifier: nullifier2,
            block_num: block_number2
        }]
    );
}

#[test]
#[miden_node_test_macro::enable_logging]
fn db_block_header() {
    let mut conn = create_db();

    // test querying empty table
    let block_number = 1;
    let res = sql::select_block_header_by_block_num(
        &conn.transaction().unwrap(),
        Some(block_number.into()),
    )
    .unwrap();
    assert!(res.is_none());

    let res = sql::select_block_header_by_block_num(&conn.transaction().unwrap(), None).unwrap();
    assert!(res.is_none());

    let res = sql::select_all_block_headers(&conn.transaction().unwrap()).unwrap();
    assert!(res.is_empty());

    let block_header = BlockHeader::new(
        1_u8.into(),
        num_to_rpo_digest(2),
        3.into(),
        num_to_rpo_digest(4),
        num_to_rpo_digest(5),
        num_to_rpo_digest(6),
        num_to_rpo_digest(7),
        num_to_rpo_digest(8),
        num_to_rpo_digest(9),
        num_to_rpo_digest(10),
        11_u8.into(),
    );
    // test insertion
    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header).unwrap();
    transaction.commit().unwrap();

    // test fetch unknown block header
    let block_number = 1;
    let res = sql::select_block_header_by_block_num(
        &conn.transaction().unwrap(),
        Some(block_number.into()),
    )
    .unwrap();
    assert!(res.is_none());

    // test fetch block header by block number
    let res = sql::select_block_header_by_block_num(
        &conn.transaction().unwrap(),
        Some(block_header.block_num()),
    )
    .unwrap();
    assert_eq!(res.unwrap(), block_header);

    // test fetch latest block header
    let res = sql::select_block_header_by_block_num(&conn.transaction().unwrap(), None).unwrap();
    assert_eq!(res.unwrap(), block_header);

    let block_header2 = BlockHeader::new(
        11_u8.into(),
        num_to_rpo_digest(12),
        13.into(),
        num_to_rpo_digest(14),
        num_to_rpo_digest(15),
        num_to_rpo_digest(16),
        num_to_rpo_digest(17),
        num_to_rpo_digest(18),
        num_to_rpo_digest(19),
        num_to_rpo_digest(20),
        21_u8.into(),
    );

    let transaction = conn.transaction().unwrap();
    sql::insert_block_header(&transaction, &block_header2).unwrap();
    transaction.commit().unwrap();

    let res = sql::select_block_header_by_block_num(&conn.transaction().unwrap(), None).unwrap();
    assert_eq!(res.unwrap(), block_header2);

    let res = sql::select_all_block_headers(&conn.transaction().unwrap()).unwrap();
    assert_eq!(res, [block_header, block_header2]);
}

#[test]
#[miden_node_test_macro::enable_logging]
fn db_account() {
    let mut conn = create_db();

    let block_num = 1.into();
    create_block(&mut conn, block_num);

    // test empty table
    let account_ids: Vec<AccountId> =
        [ACCOUNT_ID_REGULAR_PRIVATE_ACCOUNT_UPDATABLE_CODE, 1, 2, 3, 4, 5]
            .iter()
            .map(|acc_id| (*acc_id).try_into().unwrap())
            .collect();
    let res = sql::select_accounts_by_block_range(
        &conn.transaction().unwrap(),
        0.into(),
        u32::MAX.into(),
        &account_ids,
    )
    .unwrap();
    assert!(res.is_empty());

    // test insertion
    let account_id = ACCOUNT_ID_REGULAR_PRIVATE_ACCOUNT_UPDATABLE_CODE;
    let account_commitment = num_to_rpo_digest(0);

    let transaction = conn.transaction().unwrap();
    let row_count = sql::upsert_accounts(
        &transaction,
        &[BlockAccountUpdate::new(
            account_id.try_into().unwrap(),
            account_commitment,
            AccountUpdateDetails::Private,
            vec![],
        )],
        block_num,
    )
    .unwrap();
    transaction.commit().unwrap();

    assert_eq!(row_count, 1);

    let transaction = conn.transaction().unwrap();

    // test successful query
    let res =
        sql::select_accounts_by_block_range(&transaction, 0.into(), u32::MAX.into(), &account_ids)
            .unwrap();
    assert_eq!(
        res,
        vec![AccountSummary {
            account_id: account_id.try_into().unwrap(),
            account_commitment,
            block_num,
        }]
    );

    // test query for update outside the block range
    let res = sql::select_accounts_by_block_range(
        &transaction,
        block_num + 1,
        u32::MAX.into(),
        &account_ids,
    )
    .unwrap();
    assert!(res.is_empty());

    // test query with unknown accounts
    let res = sql::select_accounts_by_block_range(
        &transaction,
        block_num + 1,
        u32::MAX.into(),
        &[6.try_into().unwrap(), 7.try_into().unwrap(), 8.try_into().unwrap()],
    )
    .unwrap();
    assert!(res.is_empty());
}

#[test]
#[miden_node_test_macro::enable_logging]
fn notes() {
    let mut conn = create_db();

    let block_num_1 = 1.into();
    create_block(&mut conn, block_num_1);

    // test empty table
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[],
        &[],
        0.into(),
    )
    .unwrap();
    assert!(res.is_empty());

    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[1, 2, 3],
        &[],
        0.into(),
    )
    .unwrap();
    assert!(res.is_empty());

    let sender = AccountId::try_from(ACCOUNT_ID_PRIVATE_SENDER).unwrap();

    // test insertion
    let transaction = conn.transaction().unwrap();

    sql::upsert_accounts(&transaction, &[mock_block_account_update(sender, 0)], block_num_1)
        .unwrap();

    let note_index = BlockNoteIndex::new(0, 2).unwrap();
    let note_id = num_to_rpo_digest(3);
    let tag = 5u32;
    let note_metadata =
        NoteMetadata::new(sender, NoteType::Public, tag.into(), NoteExecutionHint::none(), ZERO)
            .unwrap();

    let values = [(note_index, note_id.into(), note_metadata)];
    let notes_db = BlockNoteTree::with_entries(values.iter().copied()).unwrap();
    let details = Some(vec![1, 2, 3]);
    let merkle_path = notes_db.get_note_path(note_index);

    let note = NoteRecord {
        block_num: block_num_1,
        note_index,
        note_id,
        metadata: NoteMetadata::new(
            sender,
            NoteType::Public,
            tag.into(),
            NoteExecutionHint::none(),
            Felt::default(),
        )
        .unwrap(),
        details,
        merkle_path: merkle_path.clone(),
    };

    sql::insert_notes(&transaction, &[(note.clone(), None)]).unwrap();
    transaction.commit().unwrap();

    // test empty tags
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[],
        &[],
        0.into(),
    )
    .unwrap();
    assert!(res.is_empty());

    // test no updates
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[tag],
        &[],
        block_num_1,
    )
    .unwrap();
    assert!(res.is_empty());

    // test match
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[tag],
        &[],
        block_num_1.parent().unwrap(),
    )
    .unwrap();
    assert_eq!(res, vec![note.clone().into()]);

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
    sql::insert_notes(&transaction, &[(note2.clone(), None)]).unwrap();
    transaction.commit().unwrap();

    // only first note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[tag],
        &[],
        block_num_1.parent().unwrap(),
    )
    .unwrap();
    assert_eq!(res, vec![note.clone().into()]);

    // only the second note is returned
    let res = sql::select_notes_since_block_by_tag_and_sender(
        &conn.transaction().unwrap(),
        &[tag],
        &[],
        block_num_1,
    )
    .unwrap();
    assert_eq!(res, vec![note2.clone().into()]);

    // test query notes by id
    let notes = vec![note, note2];
    let note_ids: Vec<RpoDigest> = notes.clone().iter().map(|note| note.note_id).collect();
    let note_ids: Vec<NoteId> = note_ids.into_iter().map(From::from).collect();

    let res = sql::select_notes_by_id(&conn.transaction().unwrap(), &note_ids).unwrap();
    assert_eq!(res, notes);

    // test notes have correct details
    let note_0 = res[0].clone();
    let note_1 = res[1].clone();
    assert_eq!(note_0.details, Some(vec![1, 2, 3]));
    assert_eq!(note_1.details, None);
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
    let block_num = 1.into();
    create_block(conn, block_num);

    let transaction = conn.transaction().unwrap();

    let account_updates = vec![mock_block_account_update(
        AccountId::try_from(ACCOUNT_ID_PRIVATE_SENDER).unwrap(),
        1,
    )];

    sql::upsert_accounts(&transaction, &account_updates, block_num).unwrap();

    let count = sql::insert_transactions(&transaction, block_num, &account_updates).unwrap();
    transaction.commit().unwrap();

    count
}

fn mock_account_code_and_storage(
    account_type: AccountType,
    storage_mode: AccountStorageMode,
    assets: impl IntoIterator<Item = Asset>,
) -> Account {
    let component_code = "\
    export.account_procedure_1
        push.1.2
        add
    end
    ";

    let component_storage = vec![
        StorageSlot::Value(Word::default()),
        StorageSlot::Value(num_to_word(1)),
        StorageSlot::Value(Word::default()),
        StorageSlot::Value(num_to_word(3)),
        StorageSlot::Value(Word::default()),
        StorageSlot::Value(num_to_word(5)),
    ];

    let component = AccountComponent::compile(
        component_code,
        TransactionKernel::testing_assembler(),
        component_storage,
    )
    .unwrap()
    .with_supported_type(account_type);

    AccountBuilder::new([0; 32])
        .account_type(account_type)
        .storage_mode(storage_mode)
        .with_assets(assets)
        .with_component(component)
        .build_existing()
        .unwrap()
}
