# Store component

This component persists the chain state in a `sqlite` database. It also stores each block's raw data as a file.

## Migrations

We have database migration support in place but don't actively use it yet. There is only the latest schema, and we reset
chain state (aka nuke the existing database) on each release.

Note that the migration logic includes both a schema number _and_ a hash based on the sql schema. These are both checked
on node startup to ensure that any existing database matches the expected schema. If you're seeing database failures on
startup its likely that you created the database _before_ making schema changes resulting in different schema hashes.

## Architecture

The store consists mainly of a gRPC server which answers requests from the RPC and block-producer components, as well as
new block submissions from the block-producer.

A lightweight background process performs database query optimisation by analysing database queries and statistics.
