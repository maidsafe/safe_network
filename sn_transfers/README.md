# sn_transfers

# Autonomi Network Token

The Autonomi Network Token (ANT) is a curency built on top of the storage layer of the Autonomi Network. It is used on the Network to store data and network nodes are rewarded with this Token for their work. ANT does not use a blockchain but a distributed Directed Acyclic Graph (DAG) of `Spend`s which are all linked together all the way to the first `Spend` which we call `Genesis`. Those `Spend`s contain transaction data and all the information necessary for verification and audit of the currency. 

## Keys

Just like many digital currencies, we use [public/private key cryptography](https://en.wikipedia.org/wiki/Public-key_cryptography) (in our case we use [bls](https://en.wikipedia.org/wiki/BLS_digital_signature) keys, implemented in the [blsttc rust crate](https://docs.rs/blsttc/latest/blsttc/)). A wallet consists of two keys:

- `MainPubkey`: equivalent to a Bitcoin address, this is used to receive ANT. It can be shared publicly. 
- `MainSecretKey`: it's the secret from which a `MainPubkey` is generated, it is used for spending ANT. 

Unlike one could expect, the `MainPubkey` itself never owns any money: `UniquePubkey`s derived from it do. Value is owned by those `UniquePubkey`s which are spendable only once in the form of a `Spend` uploaded at that `UniquePubkey`'s address (known as a `SpendAddress`) on the Network. 

The way we obtain those `UniquePubkey`s is by using bls key derivation, an algorithm which can create a new key from another key by deriving it with a `DerivationIndex`. `UniquePubkey`s are obtained by deriving the `MainPubkey`. To spend the value owned by a `UniquePubkey`, one uses the associated `DerivedSecretKey` which is obtained by deriving the `MainSecretKey` with the same `DerivationIndex` as the `UniquePubkey`'s. 

This `DerivedSecretKey` is used to sign the `Spend` which is then sent to the Network for validation and storage. Once the Network has stored and properly replicated that `Spend`, that `UniquePubkey` is considered to be spent and cannot ever be spent again. If more than one `Spend` entry exist at a given `SpendAddress` on the Network, that key is considered to be burnt which makes any `Spend` refering to it unspendable. 

Without the `DerivationIndex`, there is no way to link a `MainPubkey` to a `UniquePubkey`. Since `UniquePubkey`s are spendable only once, this means every transaction involves new and unique keys which are all unrelated and unlinkable to their original owner's `MainPubkey`.

Under the hood, those types are simply:

- `MainPubkey` => `blsttc::PublicKey`
- `UniquePubkey` => `blsttc::PublicKey`
- `MainSecretKey` => `blsttc::SecretKey`
- `DerivedSecretKey` => `blsttc::SecretKey`
- `DerivationIndex` => `u256` (impossible to guess big number)


## Spends

When a `UniquePubkey` is spent, the owner creates a `Spend` and signs it with the associated `DerivedSecretKey` before uploading it to the Network. A `Spend` contains the following information:

```rust
pub struct Spend {
    pub unique_pubkey: UniquePubkey,
    pub ancestors: BTreeSet<UniquePubkey>,
    pub descendants: BTreeMap<UniquePubkey, NanoTokens>,
}
```

A `Spend` refers to
- its own `UniquePubkey`
- its `ancestors` (which refer to it as a one of the `descendants`)
- its `descendants` (which could refer to it as one of the `ancestors`)

```go
         GenesisSpend
            /   \
       SpendA    SpendB
        /  \         \
   SpendC  SpendD    SpendE
    /        \          \
...          ...         ...
```

> All the `Spend`s on a Network come from Genesis.

Each descendant is given some of the value of the spent `UniquePubkey`. The value of a `Spend` is the sum of the values inherited from its ancestors. 

```go
               SpendS(19)                       value
              /    |    \                         |
             9     4     6                   value inherited
            /      |      \                       |
     SpendW(9)  SpendX(4)  SpendY(6)            value
       /     \     |                              |
      6       3    4                         value inherited
     /          \  |                              |
SpendQ(6)        SpendZ(7)                        V

```

> In the above example, Spend Z has 2 ancestors W and X which gave it respectively `3` and `4`. 
> Z's value is the sum of the inherited value from its ancestors: `3 + 4 = 7`.
>
> In this example `SpendW` of value `9` would look something like:
> ```
> Spend {
>    unique_pubkey = W,
>    ancestors = {S},
>    descendants = {Z : 3, Q : 6},
> }
> ```

`Spend`s on the Network are always signed by their owner (`DerivedSecretKey`) an come with that signature: 

```rust
pub struct SignedSpend {
    pub spend: Spend,
    pub derived_key_sig: Signature,
}
```

In order to be valid and accepted by the Network a Spend must:
- be addressed at the `SpendAddress` derived from its `UniquePubkey`
- refer to existing and valid ancestors that refer to it as a descendant
- refer to descendants and donate a non zero amount to them
- the sum of the donated value to descendants must be equal to the sum of the Spend's inherited value from its ancestors

## Transfers

- ...

## Spend DAG

- ...

## Wallet

Any wallet software managing ANT must hold and secure:
- the `MainSecretKey`: password encrypted on disk or hardware wallet (leaking it could result in loss of funds)
- the `DerivationIndex`es of `UniquePubkey`s it currently owns (leaking those could result in reduced anonymity)

After receiving a `Transfer`, it should:
- verify that the ancestor spends exist on the Network and are valid
- reissue the received amount to a new `UniquePubkey` by spending the received money immediately. This is necessary to prevent the original sender from burning the ancestors spends which would result in the recipient not being able to spend the money

All `DerivationIndex`es should be discarded without a trace (no cache/log) as soon as they are not useful anymore as this could result in a loss of privacy. 

