SELECT
    account_id,
    account_commitment,
    block_num,
    details
FROM
    accounts
WHERE
    account_id IN rarray(?1)
