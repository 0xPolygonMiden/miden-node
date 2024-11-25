/*
 * Selects and merges account deltas by account id and block range.
 * Note, that `block_start` is exclusive and `block_end` is inclusive.
 * Types:
 *   0: Storage scalar values
 *   1: Storage map values
 *   2: Fungible assets
 *   3: Non-fungible assets
 */

-- Selects and merges storage deltas by account id and block range (gets latest values by slot).
SELECT
    0 AS type, a.slot, NULL, a.value
FROM
    account_storage_delta_values AS a
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3 AND
    NOT EXISTS(
        SELECT 1
        FROM account_storage_delta_values AS b
        WHERE
            a.account_id = ?1 AND
            a.slot = b.slot AND
            a.block_num > b.block_num AND
            b.block_num <= ?3
    )

UNION ALL

-- Selects and merges storage map deltas by account id and block range (gets latest values by slot
-- and key).
SELECT
    1, a.slot, a.key, a.value
FROM
    account_storage_map_delta_values AS a
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3 AND
    NOT EXISTS(
        SELECT 1
        FROM account_storage_map_delta_values AS b
        WHERE
            a.account_id = ?1 AND
            a.slot = b.slot AND
            a.key = b.key AND
            a.block_num > b.block_num AND
            b.block_num <= ?3
    )

UNION ALL

-- Selects and merges fungible asset deltas by account id and block range (sums deltas by faucet).
SELECT
    2, NULL, faucet_id, SUM(delta)
FROM
    account_fungible_asset_deltas AS
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3 AND
GROUP BY
    faucet_id

UNION ALL

-- Selects and merges non-fungible asset deltas by account id and block range (gets latest actions
-- by vault key).
SELECT
    3, NULL, a.vault_key, a.is_remove
FROM
    account_non_fungible_asset_delta_actions AS a
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3 AND
    NOT EXISTS(
        SELECT 1
        FROM account_non_fungible_asset_delta_actions AS b
        WHERE
            a.account_id = ?1 AND
            a.vault_key = b.vault_key AND
            a.block_num > b.block_num AND
            b.block_num <= ?3
    )
