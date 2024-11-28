/*
 * Computes account states for the given public account IDs and block number.
 *
 * Parameters:
 *   ?1: Account IDs (array)
 *   ?2: Block number
 * Types:
 *   0: Latest accounts' details
 *   1: Accounts' nonce
 *   2: Latest storage scalar values
 *   3: Latest storage map values
 *   4: Fungible asset deltas
 *   5: Latest non-fungible assets
 */

-- Selects the latest account details for the given account IDs.
-- It is used for getting accounts' code, which are immutable by now.
SELECT
    0 AS type, account_id, NULL, NULL, details
FROM
    accounts
WHERE
    account_id IN rarray(?1)
ORDER BY
    account_id

UNION ALL

-- Selects the latest nonce for the given account IDs and block number
SELECT
    1, account_id, NULL, NULL, nonce
FROM
    account_deltas a
WHERE
    account_id IN rarray(?1) AND
    block_num <= ?2 AND
    NOT EXISTS(
        SELECT 1
        FROM account_deltas b
        WHERE
            a.account_id = b.account_id AND
            b.block_num <= ?2 AND
            a.block_num < b.block_num
    )
ORDER BY
    account_id

UNION ALL

-- Selects the latest storage delta values for the given account ID and block number
SELECT
    2, account_id, slot, NULL, value
FROM
    account_storage_delta_values a
WHERE
    account_id IN rarray(?1) AND
    block_num <= ?2 AND
    NOT EXISTS(
        SELECT 1
        FROM account_storage_delta_values b
        WHERE
            a.account_id = b.account_id AND
            b.block_num <= ?2 AND
            a.block_num < b.block_num AND
            a.slot = b.slot
    )
ORDER BY
    account_id

UNION ALL

-- Selects the latest storage map delta values for the given account ID and block number
SELECT
    3, account_id, slot, key, value
FROM
    account_storage_map_delta_values a
WHERE
    account_id IN rarray(?1) AND
    block_num <= ?2 AND
    NOT EXISTS(
        SELECT 1
        FROM account_storage_map_delta_values b
        WHERE
            a.account_id = b.account_id AND
            b.block_num <= ?2 AND
            a.block_num < b.block_num AND
            a.slot = b.slot AND
            a.key = b.key
    )
ORDER BY
    account_id

UNION ALL

-- Calculates fungible asset deltas for the given account ID and block number
SELECT
    4, account_id, NULL, faucet_id, SUM(delta) AS value
FROM
    account_fungible_asset_deltas
WHERE
    account_id IN rarray(?1) AND
    block_num <= ?2
GROUP BY
    account_id, faucet_id
HAVING
    value != 0
ORDER BY
    account_id

UNION ALL

/*
  Selects the latest non-fungible assets for the given account ID and block number.
  If asset was removed later, then it is not included.
  Here we assume that the data were validated before inserting into the DB (there is no repeated
  additions of the same assets without corresponding removals).
 */
SELECT
    5, account_id, NULL, vault_key, NULL
FROM
    account_non_fungible_asset_delta_actions a
WHERE
    account_id IN rarray(?1) AND
    block_num <= ?2 AND
    is_remove = 0 AND
    NOT EXISTS(
        SELECT 1
        FROM account_non_fungible_asset_delta_actions b
        WHERE
            a.account_id = b.account_id AND
            b.is_remove = 1 AND
            b.block_num <= ?2 AND
            a.vault_key = b.vault_key AND
            a.block_num < b.block_num
    )
ORDER BY
    account_id
