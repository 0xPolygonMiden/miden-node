/*
 * Selects and merges account deltas by account id and block range.
 * Note, that `block_start` is exclusive and `block_end` is inclusive.
 * Parameters:
 *   ?1: Account ID
 *   ?2: Block start
 *   ?3: Block end
 * Types:
 *   0: Storage scalar values
 *   1: Storage map values
 *   2: Fungible assets
 *   3: Non-fungible assets
 */

-- Selects and merges storage deltas by account id and block range (gets latest values by slot).
SELECT
    0 AS type, block_num, slot, NULL, value
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
            b.account_id = ?1 AND
            a.slot = b.slot AND
            a.block_num < b.block_num AND
            b.block_num <= ?3
    )

UNION ALL

-- Selects and merges storage map deltas by account id and block range (gets latest values by slot
-- and key).
SELECT
    1, block_num, a.slot, a.key, a.value
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
            b.account_id = ?1 AND
            a.slot = b.slot AND
            a.key = b.key AND
            a.block_num < b.block_num AND
            b.block_num <= ?3
    )

UNION ALL

-- Selects and merges fungible asset deltas by account id and block range (sums deltas by faucet).
SELECT
    2, block_num, NULL, faucet_id, SUM(delta)
FROM
    account_fungible_asset_deltas
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3
GROUP BY
    faucet_id

UNION ALL

-- Selects and merges non-fungible asset deltas by account id and block range (gets latest actions
-- by vault key).
SELECT
    3, block_num, NULL, vault_key, is_remove
FROM
    account_non_fungible_asset_delta_actions
WHERE
    account_id = ?1 AND
    block_num > ?2 AND
    block_num <= ?3
ORDER BY
    block_num
