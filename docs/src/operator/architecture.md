# Node architecture

The node itself consists of three distributed components: store, block-producer and RPC.

The components can be run on separate instances when optimised for performance, but can also be run as a single process
for convenience. At the moment both of Miden's public networks (testnet and devnet) are operating in single process
mode.

The inter-component communication is done using a gRPC API wnich is assumed trusted. In other words this _must not_ be
public. External communication is handled by the RPC component with a separate external-only gRPC API.

```mermaid
---
title: Infrastructure architecture example
---
graph LR;
    
  subgraph RPC
  direction RL
    rpc_a[RPC A]
    rpc_b[RPC B]
    rpc_i[......]
    rpc_n[RPC N]
  end

  subgraph internal
  direction TB
    store
    block-producer
  end

  load_balancer[Load Balancer]

  load_balancer ---> RPC

  RPC -- tx   ---> block-producer 
  RPC -- query --> store

  block-producer -- block --> store
```

## RPC

The RPC component provides a public gRPC API with which users can submit transactions and query chain state. Queries are
validated and then proxied to the store. Similarly, transaction proofs are verified before submitting them to the
block-producer. This takes a non-trivial amount of load off the block-producer.

This is the _only_ external facing component and it essentially acts as a shielding proxy that prevents bad requests
from impacting block production.

It can be trivially scaled horizontally e.g. with a load-balancer in front as shown above.

## Store

The store is responsible for persisting the chain state. It is effectively a database which holds the current state of
the chain, wrapped in a gRPC interface which allows querying this state and submitting new blocks.

It expects that this gRPC interface is _only_ accessible internally i.e. there is an implicit assumption of trust.

## Block-producer

The block-producer is responsible for aggregating received transactions into blocks and submitting them to the store.

Transactions are placed in a mempool and are periodically sampled to form batches of transactions. These batches are
proved, and then periodically aggregated into a block. This block is then proved and committed to the store.
