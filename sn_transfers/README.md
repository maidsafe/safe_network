# sn_transfers

The `sn_transfers` crate is responsible for managing transfers within the Safe Network. 

Its main component is the [`CashNote`](https://github.com/maidsafe/sn_transfers/blob/main/src/cash_note.rs), which forms the basis of transfers on the Safe Network.

A `CashNote` represents a spendable unit of currency in the network, denoting ownership of a certain number of `NanoTokens`.

To execute a transfer, a [`SignedSpend`](https://github.com/maidsafe/sn_transfers/blob/main/src/signed_spend.rs) needs to be created and validated on the network.

`Transfer`s are directed to `UniquePubKey`s, which are derived from `MainPubKey`s. Using a `DerivationIndex`, the recipient can generate the necessary `SecretKey` to spend the `NanoTokens`.

Since most of the required information is stored in a `SignedSpend` on the network, we also provide a [`CashNoteRedemption`](https://github.com/maidsafe/sn_transfers/blob/main/src/cash_note_redemption.rs) struct. This struct contains the minimum information needed to retrieve a full `Spend` from the network and generate the keys required to spend the `NanoTokens`.

For error handling, we expose [`Error`](https://github.com/maidsafe/sn_transfers/blob/main/src/error.rs) and [`Result`](https://github.com/maidsafe/sn_transfers/blob/main/src/result.rs) types.

Additionally, this crate re-exports the `bls` crate used in the public API and includes a helper module for creating an Rng when invoking `sn_transfers` methods that require them.
