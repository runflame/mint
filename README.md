# Mint

This implements the sidechain minting algorithm as described in [1](https://telegra.ph/Minting-sidechains-market-driven-extensions-to-Bitcoin-03-11).

## API overview

The API consists of two parts: **tracker** and **mint**. 

### Tracker

* follows the most-weight sidechain,
* handles Bitcoin reorganizations, 
* handles sidechain reorganizations,
* provides tx fee data for fee estimation used by the Mint,
* provides the minting price feed as a rolling window of 144 blocks (≈24 hrs).

### Mint

* keeps a balance of Bitcoins,
* prepares minting transactions,
* assembles the sidechain blocks.

This library does not handle transport (p2p networking) or specifics of the sidechain blocks and transaction format.
These are provided by the user of the library.

## Protocol overview

The Bitcoin chain is used as a proof of weight,
but does not contain the source data required to verify the sidechain blocks.

At the same time, in contrast to Bitcoin’s proof-of-work, the sidechain blocks do not have a self-contained weight metric:
their “proof of weight” is tied to a current valid chain of Bitcoin blocks.

Since we have two chains we are present with more reorganization possibilities: 

1. Sidechain may be reorged if new blocks appear that have
   a higher cumulative weight than the existing chain.
2. When Bitcoin chain reorgs, sidechain mint transactions
   could be relocated to new blocks, but not lead to a sidechain reorg.
3. Alternatively, if Bitcoin chain reorgs, some mint transactions
   may disappear or get delayed, which affects the cumulative weight of the sidechain
   and could lead to a sidechain reorg.

On the bright side, the Utreexo state helps keeping a copy of the entire blockchain state at each block.
This allows us to pre-validate independent subgraphs of the chains without mutating a global state, 
and recompute their weights at any time to select the best chain.

