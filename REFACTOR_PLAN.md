# Zebra Simplification Plan

Static-analysis survey of the workspace at `801e3bf0a` (branch `refactor/simplify`).
No code was modified to produce this document; nothing was compiled.

**Goal:** remove unnecessary code — fewer lines, fewer files, less indirection — with
**zero behaviour change**. No consensus rule logic, no serialization compatibility
shims, and no tests are proposed for removal.

## Workspace size (src/ only, excludes tests/ dirs at crate root)

| Crate | LOC | Files |
| --- | ---: | ---: |
| zebra-chain | 41,327 | 206 |
| zebra-state | 38,379 | 94 |
| zebra-network | 31,248 | 100 |
| zebrad | 30,230 | 78 |
| zebra-rpc | 23,230 | 66 |
| zebra-consensus | 16,464 | 36 |
| zebra-test | 16,420 | 17 |
| zebra-script | 2,115 | 2 |
| tower-batch-control | 1,131 | 7 |
| zebra-utils | 906 | 6 |
| zebra-node-services | 730 | 8 |
| tower-fallback | 308 | 3 |

## Method

Three signals were combined:

1. **`#[allow(dead_code)]` census** — 178 sites. For each, the annotated item name was
   counted across every `.rs` file in the workspace. A count of exactly 1 means the
   name appears only on its own definition line.
2. **Unused `pub` item scan** — 2,693 `pub`/`pub(crate)` `fn`/`struct`/`enum`/`trait`/
   `const`/`type` declarations in non-test files, joined against a workspace-wide
   identifier frequency index (23,203 distinct identifiers).
3. **Duplicate-window detection** — every 8-line sliding window in every non-test source
   file, normalised for whitespace, grouped by exact match. This surfaced the
   `ToHex`/`FromHex` cluster (13 sites), the batch-verifier cluster (5 sites), and the
   `miner.rs` Tower-bound cluster (4 sites).

### Two false positives, already excluded

These looked dead but are not. They are recorded so nobody re-derives them:

- **`zebra-rpc/src/methods.rs:144-172`, the 27 `PARAM_*_DESC` consts.** Zero code
  references, but `zebra-rpc/build.rs:159` calls `openrpsee::generate_openrpc` on
  `src/methods.rs`, which resolves these **by name convention** from the RPC method
  parameter names. The tell is `PARAM__ALLOW_HIGH_FEES_DESC` (double underscore) mapping
  to the `_allow_high_fees` parameter. **Do not remove.**
- **`zebra-grpc/CHANGELOG.md` and `zebra-scan/CHANGELOG.md`**, the only two files left in
  otherwise-deleted crate directories. `.changie.yaml:23` documents that they are
  deliberately retained historical records. **Do not remove.**

### Semver caveat that applies to slices 1-8

Every Zebra crate is published to crates.io. Removing a `pub` item is a breaking change
even when nothing in-tree uses it, and per `CLAUDE.md` that needs the `!` marker on both
the PR title and the commit that introduces it (the PR gate reads the title, release-plz
reads the commits). Two ways to handle this, pick one before starting:

- **Preferred:** land the removals as one `refactor!:` commit per crate, so release-plz
  emits a single major bump per crate rather than a trickle.
- **Lower friction:** for items that are genuinely internal, downgrade `pub` →
  `pub(crate)` first (non-breaking for the ones already unreachable from outside), let
  `dead_code` fire, and delete in a follow-up. Costs an extra round trip.

Slices 9+ are internal-only or additive and carry no semver cost.

---

## Slices

Ordered lowest-risk/highest-impact first. LOC deltas are counted from the actual line
ranges in the tree, including doc comments, and are net of any helper added.

---

### 1. Delete confirmed-dead functions in `zebra-state`

**Target files**

- `zebra-state/src/service/non_finalized_state.rs` — `any_prev_block_hash_for_hash`
  (:753), `best_height_by_hash` (:788), `any_height_by_hash` (:796). Removing the first
  orphans `any_block_by_hash` (:736), whose only caller it is — delete that too.
- `zebra-state/src/service/non_finalized_state/chain.rs` — `transaction_by_loc` (:490),
  `transaction_hash_by_loc` (:501), `contains_hash_or_height` (:553),
  `non_finalized_nth_hash` (:1448).
- `zebra-state/src/service/block_iter.rs` — `known_chain_ancestor_iter` (:191).
- `zebra-state/src/service/finalized_state/zebra_db/block.rs` —
  `prev_block_hash_for_hash` (:108), `prev_block_height_for_hash` (:117).
- `zebra-state/src/service/finalized_state/zebra_db/shielded.rs` —
  `contains_sprout_anchor` (:122), `delete_range_sapling_tree` (:818),
  `delete_range_orchard_tree` (:950), `delete_orchard_anchor` (:962).
- `zebra-state/src/service/finalized_state/disk_format/transparent.rs` —
  `address_iterator_next` (:570).
- `zebra-state/src/service/finalized_state/disk_format/upgrade.rs` —
  `is_run_at_startup` (:281).
- `zebra-state/src/service/finalized_state/column_family.rs` —
  `take_batch_for_writing` (:133).

**What to do** Delete each function and its `#[allow(dead_code)]`. Work bottom-up per
file so line numbers stay valid. After each file, re-check whether a private helper the
deleted function called is now itself unreferenced (the `any_block_by_hash` case above is
the known one; there may be one or two more).

The `delete_*` helpers on `ZebraDb` are write-path methods with no callers. They are not
part of any migration — the `DiskFormatUpgrade` implementors in `disk_format/upgrade/`
use their own batch writes. Confirm that per method before deleting.

**Est. LOC delta** −280

**Risk** Low. Nothing calls these. The one thing to watch is that
`shielded.rs`'s `delete_*` methods are not reachable through a `DiskFormatUpgrade` impl
via a trait object; grep `disk_format/upgrade/` for each name before removing.

**Verify** `cargo check -p zebra-state --all-targets`, then
`cargo test -p zebra-state`. `--all-targets` matters here: several of these are within
one grep-hop of test-only helpers.

---

### 2. Delete confirmed-dead items in `zebra-network`

**Target files**

- `zebra-network/src/protocol/external/codec.rs` — `for_version` (:110).
- `zebra-network/src/protocol/external/inv.rs` — `from_legacy_tx_id` (:67).
- `zebra-network/src/protocol/internal/response.rs` — `is_nil` (:157).
- `zebra-network/src/protocol/internal/response_status.rs` — `map_available` (:51),
  `map_missing` (:61). See slice 15 for the rest of this file.
- `zebra-network/src/peer/handshake.rs` — `new_inbound_proxy` (:237).
- `zebra-network/src/address_book.rs` — `pending_reconnection_addr` (:619).
- `zebra-network/src/meta_addr.rs` — `ping_sent` (:944).

**What to do** Delete each with its `#[allow(dead_code)]`.

`from_legacy_tx_id` is worth a second look before deleting: it constructs an inventory
hash from a pre-v5 txid. Confirm the v5 path (`InventoryHash::Tx` vs `Wtx`) does not need
it for a peer speaking an older protocol version — check
`zebra-network/src/constants.rs` for the minimum supported version. If there is any
doubt, drop this one item and keep the rest of the slice.

**Est. LOC delta** −95

**Risk** Low, with the `from_legacy_tx_id` caveat above.

**Verify** `cargo check -p zebra-network --all-targets`,
`cargo test -p zebra-network`.

---

### 3. Delete confirmed-dead items in `zebra-consensus`

**Target files**

- `zebra-consensus/src/checkpoint.rs` — `CheckpointVerifier::new` (:204) and
  `CheckpointVerifier::from_list` (:232). Both are `#[allow(dead_code)]`. The live
  construction path is `from_checkpoint_list`, which the router calls directly.
- `zebra-consensus/src/checkpoint/types.rs` — `Progress::is_before_genesis` (:80).
- `zebra-consensus/src/primitives/groth16.rs` — `BatchVerifyingKey` type alias (:62).

**What to do** Delete. `CheckpointVerifier::new` has a substantial doc comment
explaining the "call once per network" invariant — move that prose onto
`from_checkpoint_list`, which is where it actually applies now, rather than dropping it.

**Est. LOC delta** −70

**Risk** Low. `CheckpointVerifier::new` is `pub` — semver, see the caveat above.

**Verify** `cargo check -p zebra-consensus --all-targets`,
`cargo test -p zebra-consensus`.

---

### 4. Delete confirmed-dead items in `zebra-chain`

**Target files**

- `zebra-chain/src/error.rs` — `NoteCommitmentError` (:30) and `KeyError` (:42). Both
  are single-variant enums wrapping `RandError`, and neither name appears anywhere else
  in the workspace. `AddressError` and `NoteError` (same shape) *are* used — keep them.
- `zebra-chain/src/serialization/date_time.rs` — `checked_elapsed` (:76).
- `zebra-chain/src/work/difficulty.rs` — `difficulty_bits_for_display` (:702),
  `difficulty_multiplier_for_display` (:684). Check `zebra-rpc` first: these read like
  they were written for `getblockchaininfo`/`getmininginfo` and the RPC may have grown
  its own copy. If so, this is a duplication fix rather than a deletion — unify instead.
- `zebra-chain/src/transparent/utxo.rs` — `from_location` (:74), `from_utxo` (:109).
- `zebra-chain/src/transparent.rs` — `value_from_ordered_utxos` (:292).
- `zebra-chain/src/sapling/output.rs` — `from_v4` (:75).
- `zebra-chain/src/transaction.rs` — `has_transparent_inputs_or_outputs` (:220).
- `zebra-chain/src/transaction/lock_time.rs` — `is_time` (:98).
- `zebra-chain/src/transaction/joinsplit.rs` — `joinsplits_mut` (:85).
- `zebra-chain/src/parameters/network_upgrade.rs` — `previous_upgrade` (:337).
- `zebra-chain/src/orchard/shielded_data.rs` — `proof_size_is_canonical` (:121).
- `zebra-chain/src/history_tree.rs` — `try_extend` (:271).

**What to do** Delete. `zebra-chain` is the most-depended-on crate and the most
semver-sensitive; land this as one `refactor(zebra-chain)!:` commit.

**Explicitly excluded from this slice:** the unused version-group-ID constants in
`zebra-chain/src/parameters/transaction.rs` (`OVERWINTER_VERSION_GROUP_ID`,
`SAPLING_VERSION_GROUP_ID`, `TX_V5_VERSION_GROUP_ID`, `TX_V6_VERSION_GROUP_ID`). They
are unreferenced, but they document the wire format and fall under the
"no serialization compatibility shims" rule. Leave them.

`proof_size_is_canonical` also deserves a second look — a canonical-encoding predicate is
the kind of thing that is dead because a consensus check was refactored, not because it
was never needed. Confirm the equivalent check exists elsewhere before deleting.

**Est. LOC delta** −160

**Risk** Low-medium. Low mechanically; medium because it is the widest-blast-radius
crate and two items (`proof_size_is_canonical`, the `difficulty_*_for_display` pair)
need a judgement call first.

**Verify** `cargo check --workspace --all-targets --locked` — a `zebra-chain` change can
break any crate. Then `cargo test -p zebra-chain`.

---

### 5. Delete confirmed-dead items in `zebra-rpc`

**Target files**

- `zebra-rpc/build.rs:13-14` — `ZAINO_COMMIT`, already `#[allow(dead_code)]` and
  already carrying a `TODO: Zaino is not currently built by this build script.` The
  commit hash is worth keeping as a comment; the `const` is not.
- `zebra-rpc/src/indexer.rs` — `try_into_hash_and_height` (:30).
- `zebra-rpc/src/methods/types/get_block_template.rs` — `try_into_proposal` (:442).
- `zebra-rpc/src/methods.rs` — `new_valid` (:3857).

**What to do** Delete. Keep the Zaino commit hash as a plain comment.

`try_into_proposal` is on the block-template path; confirm the `getblocktemplate`
proposal mode goes through a different conversion before removing it.

**Est. LOC delta** −55

**Risk** Low.

**Verify** `cargo check -p zebra-rpc --all-targets`, `cargo test -p zebra-rpc`.

---

### 6. Replace `#[allow(dead_code)]` with `#[cfg(test)]` on test-only helpers

**Target files** Roughly 40 of the 178 `#[allow(dead_code)]` sites are on items whose
*only* callers live in test modules. Confirmed examples:

- `zebra-state/src/service/non_finalized_state.rs` — `best_contains_sprout_nullifier`
  (:808), `best_contains_sapling_nullifier` (:817), `best_contains_orchard_nullifier`
  (:833). All three are called only from
  `zebra-state/src/service/check/tests/nullifier.rs`.
- `zebra-state/src/service/non_finalized_state.rs` — `best_hash` (:769), `best_tip`
  (:779 region), used only from `service/read/*/tests/vectors.rs`.
- `zebra-state/src/service/non_finalized_state/chain.rs` — `recent_fork_height` (:342),
  `recent_fork_length` (:350).

**What to do** Swap `#[allow(dead_code)]` for
`#[cfg(any(test, feature = "proptest-impl"))]` (matching the crate's existing convention
— `zebra-chain/src/work.rs` and `chain_sync_status.rs` already gate this way). This is
line-neutral but removes these from the release build *and* makes the remaining
`#[allow(dead_code)]` sites meaningful: after slices 1-5 and this one, any surviving
`allow(dead_code)` is genuinely suspicious.

Do not delete these — they are test infrastructure.

**Est. LOC delta** 0 (removes ~40 items from release builds)

**Risk** Low. The failure mode is a compile error, not a silent behaviour change.

**Verify** `cargo check -p zebra-state` (release path — must still compile with the
items gone) *and* `cargo check -p zebra-state --all-targets` (test path — must still
see them). Both are required; either alone misses half the change.

---

### 7. Collapse `miner.rs`'s expanded Tower bounds onto the existing trait aliases

**Target file** `zebrad/src/components/miner.rs`

**What to do** The file has four fully-expanded `where` blocks — lines 67-104, 119-156,
238-275, 365-402, ~37 lines each, 148 total — of this shape:

```rust
Mempool: Service<mempool::Request, Response = mempool::Response,
                 Error = zebra_node_services::BoxError>
    + Clone + Send + Sync + 'static,
Mempool::Future: Send,
State: Service<zebra_state::Request, Response = zebra_state::Response,
               Error = zebra_state::BoxError>
    + Clone + Send + Sync + 'static,
<State as Service<zebra_state::Request>>::Future: Send,
// ... and so on
```

The trait aliases that express exactly this already exist and are already used by
`zebra-rpc/src/methods.rs:844-851`:

- `zebra_node_services::mempool::service_trait::MempoolService`
- `zebra_state::service::traits::{State, ReadState}`
- `zebra_consensus::router::service_trait::BlockVerifierService`

all built on `zebra_node_services::service_traits::ZebraService`. `miner.rs` predates
their adoption and was never migrated. Rewrite each of the four blocks to the 7-line
form `methods.rs` uses.

This is the clearest instance of a general pattern: 23 sites across `zebra-rpc/src`,
`zebrad/src`, and `zebra-consensus/src` still spell out `Error = zebra_*::BoxError`
inline. `miner.rs` holds 8 of them and is by far the densest; sweep the rest
opportunistically once this lands.

**Est. LOC delta** −120

**Risk** Low. The aliases are blanket-implemented (`impl<T> State for T where T: ...`),
so this is bound-for-bound identical, and any mismatch is a compile error.

**Verify** `cargo check -p zebrad --features internal-miner --all-targets`. The
`internal-miner` feature gate is mandatory — `components.rs:22` puts the whole module
behind it, so a default `cargo check` will not compile this file at all.

---

### 8. Delete the unused `MappedRequest` scaffolding in `zebra-state`

**Target file** `zebra-state/src/request.rs:636-780`

**What to do** `MappedRequest` is a helper trait with five implementors. Four are
`#[allow(dead_code)]` and unreferenced:

- `CommitCheckpointVerifiedBlockRequest` (:694)
- `InvalidateBlockRequest` (:715)
- `ReconsiderBlockRequest` (:737)
- `AwaitUtxoRequest` (:759)

Only `CommitSemanticallyVerifiedBlockRequest` (:673) is used. Each dead implementor is
~22 lines of `map_request`/`map_response` boilerplate whose `map_response` arm is a
`match` with an `unreachable!` fallback.

Delete the four. Then decide on the trait itself: with one implementor left,
`MappedRequest` is a single-impl trait and `mapped_oneshot` could become an inherent
method on `CommitSemanticallyVerifiedBlockRequest`. That removes the trait, its
`async_fn_in_trait` allow, and the `Sized + Send + 'static` bounds — another ~30 lines.

Recommendation: delete the four implementors now, and keep the trait only if there is a
known plan to add implementors back. The `unreachable!` pattern in `map_response` says
this was speculative scaffolding rather than a design in use.

**Est. LOC delta** −110 (−140 if the trait goes too)

**Risk** Low.

**Verify** `cargo check -p zebra-state --all-targets`, `cargo test -p zebra-state`.

---

### 9. Macro-ise the `ToHex`/`FromHex`/`Display`/`Debug` boilerplate in `zebra-chain`

**Target files** 29 `impl ToHex` blocks across 12 files:

`block/hash.rs` (:52, :62), `block/commitment.rs` (:214, :224, :324, :334),
`block/merkle.rs` (:104, :114, :271, :281), `transaction/hash.rs` (:115, :125),
`transaction/auth_digest.rs` (:63, :73), `transparent/script.rs` (:65, :75),
`work/equihash.rs` (:304, :314), `work/difficulty.rs` (:359, :369, :524, :534),
`sapling/commitment.rs` (:46), `sapling/tree.rs` (:109, :119),
`orchard/tree.rs` (:233, :243), `parameters/network_upgrade.rs` (:176, :186).

**What to do** Every one of these types repeats the same four-impl block, verbatim
modulo the type name:

```rust
impl ToHex for &T { /* delegates to bytes_in_display_order().encode_hex() */ }
impl ToHex for T  { /* delegates to (&self) */ }
impl FromHex for T { /* <[u8; 32]>::from_hex, then reverse or from_bytes_in_display_order */ }
impl fmt::Display / fmt::Debug for T { /* f.write_str(&self.encode_hex::<String>()) */ }
```

Add one `macro_rules! impl_hex_display` in `zebra-chain/src/serialization/` and invoke
it per type. The workspace already uses this idiom — `at_least_one!` in
`serialization/constraint.rs`, `args!` in `zebra-test/src/command/arguments.rs`.

**Two variants must be preserved distinctly**, and getting this wrong silently reverses
hex output on the RPC surface:

- `block::Hash`-style: `FromHex` calls `Self::from_bytes_in_display_order(&hash)`.
- `ChainHistoryMmrRootHash`-style: `FromHex` does `hash.reverse()` then `.into()`.

Give the macro an explicit byte-order argument rather than defaulting one of them.
`Debug` also differs per type — some use `f.debug_tuple("block::Hash")`, others
`write_str`. Parameterise the label.

**Est. LOC delta** −250 (about 340 lines of impls replaced by a ~60-line macro plus 29
invocations)

**Risk** Medium. Mechanically safe, but these types are the display surface for txids
and block hashes across the whole RPC API, and a byte-order slip produces
plausible-looking-but-reversed hex rather than a compile error.

**Verify** `cargo test -p zebra-chain` for the round-trip properties, then
`cargo test -p zebra-rpc` — the snapshot tests in
`zebra-rpc/src/methods/tests/snapshot.rs` (1,461 lines) pin the exact hex strings and
are the real safety net here. Do not land this slice without a clean `zebra-rpc`
snapshot run.

---

### 10. Unify the three near-identical signature batch verifiers

**Target files**

- `zebra-consensus/src/primitives/ed25519.rs` (232 lines)
- `zebra-consensus/src/primitives/redjubjub.rs` (228 lines)
- `zebra-consensus/src/primitives/redpallas.rs` (245 lines)

**What to do** `diff redjubjub.rs redpallas.rs` is 85 lines; `diff ed25519.rs
redjubjub.rs` is 71. Across 705 lines of source, the real differences are:

- the imported `batch` module (`zebra_chain::primitives::{ed25519,redjubjub,redpallas}`),
- the metric/trace label string (`"ed25519"`, `"redjubjub"`, `"redpallas"`),
- the `From` impls for `Item` — `ed25519.rs` has two concrete impls, the other two use a
  blanket `impl<T: Into<batch::Item>>,
- minor inconsistencies that are almost certainly accidental: `redjubjub.rs:184` traces
  `"got item"` where the others trace `"got <name> item"`, and its panic message at :206
  omits the scheme name.

Everything else — `Verifier`, `Default`, `take`, the `Service` impl, the `Fallback` +
`Batch` static, `flush_blocking`, `verify_single_spawning` — is identical.

Extract a single generic verifier module parameterised over a small trait carrying the
batch type, the item type, and the label. Or, if the associated-type plumbing on
`batch::Verifier` makes the generic version harder to read than the duplication (a real
possibility — check `zebra_chain::primitives::*::batch` for a common trait first), use a
`macro_rules!` instead. Prefer whichever reads better; the point is readability, not
cleverness.

Note that `sapling.rs` (231) and `halo2.rs` (529) share the *shape* but not the
mechanics — different item types and proof systems. Leave them alone. `groth16.rs` (207)
likewise.

**Fix the trace-message inconsistencies as part of this**, since unification forces a
single wording anyway. Metric *names* (`signatures.ed25519.validated` etc.) must stay
byte-identical — they are an observability contract.

**Est. LOC delta** −400

**Risk** Medium. No consensus rule logic moves, but this is the signature verification
path, and the batch/fallback interaction (batch failure → per-item re-verification) is
subtle. The `Fallback` static's unnameable-closure cast at `ed25519.rs:87-89` is the
fiddly part to generify.

**Verify** `cargo test -p zebra-consensus` — each module has its own `mod tests` with
batch-failure cases. Additionally run `cargo test -p zebra-consensus -- --ignored` if
those tests carry an ignore gate; the fallback path is what regresses silently here.

---

### 11. Macro-ise the fixed-size `IntoDisk`/`FromDisk` impls in `zebra-state`

**Target files** 64 impls across
`zebra-state/src/service/finalized_state/disk_format/{shielded,transparent,block,chain}.rs`
and `disk_format.rs` (22/17/15/6/4 respectively).

**What to do** A large fraction are exactly this pair:

```rust
impl IntoDisk for T {
    type Bytes = [u8; 32];
    fn as_bytes(&self) -> Self::Bytes { self.into() }   // or *self.0, or (*self).into()
}
impl FromDisk for T {
    fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        let array: [u8; 32] = bytes.as_ref().try_into().unwrap();
        array.into()
    }
}
```

`shielded.rs:20-80` alone has five consecutive instances. A
`impl_disk_via_array!(T, 32)` macro would cover the `[u8; 32]` newtype cases.

**Scope this narrowly.** Only fold in impls that are *byte-for-byte* the delegating form
above. Anything with a bincode call, a length prefix, a manual `Ord`-preserving
encoding, or a comment explaining the encoding choice (e.g. `ironwood::Nullifier` at
`shielded.rs:44-51`, which documents *why* it reuses Orchard's encoding) stays written
out. That comment is load-bearing and must survive verbatim wherever the impl lands.

This is a serialization compatibility layer. The macro must not change a single output
byte.

**Est. LOC delta** −150

**Risk** Medium-high. Getting this wrong corrupts the on-disk database format, and
`disk_format.rs`'s header warns that
`state_database_format_version_in_code()` must be bumped on any format change — which is
precisely the thing that must *not* be needed here. If it turns out a bump would be
required, the refactor is wrong; back it out.

**Verify** `cargo test -p zebra-state` — the snapshot tests
(`disk_format/tests/snapshot.rs`, `zebra_db/block/tests/snapshot.rs`) and the round-trip
proptests (`disk_format/tests/prop.rs`, 490 lines) both pin exact bytes. Do not land
without both green. Consider this the lowest-priority slice in the plan relative to its
risk — it is listed for completeness, and skipping it costs 150 lines out of ~2,000.

---

### 12. Remove the single-impl `With<T>` trait in `zebrad`

**Target files** `zebrad/src/components.rs:29-33` (definition),
`zebrad/src/config.rs:191` (its only impl, `With<MinerAddressType> for ZebradConfig`).

**What to do** A one-method trait with exactly one implementor, defined in
`components.rs` but implemented in `config.rs` — so a reader hitting `.with(...)` in
`config.rs` has to jump modules to learn it is a plain builder setter. Replace with an
inherent method on `ZebradConfig`.

Name it for what it does (`with_miner_address`), not `with` — the generic name only
existed to satisfy the trait.

Careful when grepping: `.with(` collides heavily with `tracing_subscriber`'s layer
builder in `components/tracing/component.rs`, which is unrelated.

**Est. LOC delta** −10

**Risk** Low.

**Verify** `cargo check -p zebrad --all-targets`, `cargo test -p zebrad --lib`.

---

### 13. Fold the trivial `error.rs` into its parent module

**Target file** `zebra-chain/src/block/error.rs` (10 lines)

**What to do** The whole file is one single-variant enum:

```rust
pub enum BlockError { WrongTransactionConsensusBranchId }
```

Move it into `zebra-chain/src/block.rs` and delete the file. One fewer file, one fewer
`mod` line, one fewer navigation hop.

Keep the type and its variant exactly as-is — it is a consensus error and its
`#[error(...)]` string may be matched by tests or surfaced to RPC clients. Re-export from
the same path so no downstream import changes.

**Est. LOC delta** −8, −1 file

**Risk** Low.

**Verify** `cargo check --workspace --locked`.

---

### 14. Audit the remaining `#[allow(dead_code)]` sites

**Target** Whatever survives slices 1-6 — roughly 90 of the original 178.

**What to do** Not a mechanical slice; a triage pass. Each survivor is one of:

- genuinely used only under a non-default feature — replace with the right `cfg`,
- used only by a `proptest-impl` consumer — replace with the existing
  `cfg(any(test, feature = "proptest-impl"))`,
- still actually dead, but a cascade only reachable after an earlier slice lands.

Highest-density remaining files, worth doing first:
`zebrad/src/components/mempool/storage.rs` (9 sites),
`zebra-network/src/peer_set/inventory_registry.rs` (4),
`zebra-state/src/service/finalized_state/disk_format/transparent.rs` (4).

The goal is that a bare `#[allow(dead_code)]` becomes rare enough to be a code smell on
sight. Re-run the frequency scan from the Method section after slices 1-6 to regenerate
the list — several counts will have dropped to 1 by cascade.

**Est. LOC delta** −100 (speculative; depends on triage outcomes)

**Risk** Low per item, but this is many small independent judgement calls. Do it as
several small commits, not one.

**Verify** Per-crate `cargo check --all-targets` and `cargo test`, plus a bare
`cargo check -p <crate>` to confirm the release path still builds.

---

### 15. Trim the unused combinators on `InventoryResponse`

**Target file** `zebra-network/src/protocol/internal/response_status.rs` (96 lines)

**What to do** This type reimplements `Result`'s combinator surface. Actual usage:

| Method | In-tree uses |
| --- | ---: |
| `map_available` | 0 |
| `map_missing` | 0 |
| `is_missing` | 4 (grep-inflated; likely fewer) |
| `is_available` | 8 (grep-inflated) |
| `command`, `as_ref`, `available`, `missing` | ambiguous — names collide workspace-wide |

`map_available`/`map_missing` are covered by slice 2. For the rest, resolve the counts
properly first: `available`, `missing`, `command`, and `as_ref` are common English words
and common method names, so the raw greps (145/227/180/88) are meaningless. Use a
type-aware search — grep for the receiver expressions, or just delete-and-compile one at
a time.

Delete whatever is genuinely unused. Keep `available`/`missing`/`as_ref` if used — they
are the type's reason for existing.

**Est. LOC delta** −30

**Risk** Low. Compile errors catch mistakes.

**Verify** `cargo check -p zebra-network --all-targets`,
`cargo test -p zebra-network`.

---

### 16. Deduplicate the `where` block repeated 6× in `zebra-rpc/src/methods.rs`

**Target file** `zebra-rpc/src/methods.rs` at :844, :906, :929, :1024, :3436, :4982

**What to do** Unlike `miner.rs` (slice 7), this file already uses the compact aliases —
but still repeats the same 7-line block six times:

```rust
where
    Mempool: MempoolService,
    State: StateService,
    ReadState: ReadStateService,
    Tip: ChainTip + Clone + Send + Sync + 'static,
    AddressBook: AddressBookPeers + Clone + Send + Sync + 'static,
    BlockVerifierRouter: BlockVerifierService,
    SyncStatus: ChainSyncStatus + Clone + Send + Sync + 'static,
{
```

Three of the seven bounds are still spelled out. Add two aliases beside the existing
ones — `ChainTipService: ChainTip + Clone + Send + Sync + 'static` and
`AddressBookService: AddressBookPeers + Clone + Send + Sync + 'static` — plus one for
`ChainSyncStatus`, following the `ZebraService` blanket-impl pattern in
`zebra-node-services/src/service_traits.rs`.

Rust has no true trait aliases on stable, so the seven generic parameters still have to
be listed at each site; this shortens each line rather than removing the block. Net win
is modest and mostly readability.

**Est. LOC delta** −25

**Risk** Low. Blanket impls make it bound-for-bound identical.

**Verify** `cargo check -p zebra-rpc --all-targets`.

---

### 17. Flatten single-`mod`-declaration files

**Target files**

- `zebra-chain/src/parallel.rs` (3 lines — only `pub mod tree;`)
- `zebra-chain/src/parameters/checkpoint.rs` (4 lines)
- `zebra-rpc/src/config.rs` (4 lines)
- `zebra-state/src/service/read/address.rs` (5 lines)
- `zebra-network/src/protocol/types.rs` (4 lines — only two `pub use` re-exports)

**What to do** Each is a Rust-2018-style module file containing nothing but `mod`
declarations for its sibling directory.

Be honest about the value here: this is idiomatic Rust, and flattening it changes public
paths (`zebra_chain::parallel::tree` → `zebra_chain::parallel_tree`), which is a
breaking change for a handful of lines. **Recommend doing only `zebra-chain/src/parallel.rs`**,
where the wrapper genuinely adds nothing (one child, no re-exports), and leaving the rest.

Listed for completeness. Lowest value-per-unit-churn in the plan; skip if time is short.

**Est. LOC delta** −3, −1 file (if scoped to `parallel.rs` as recommended)

**Risk** Low, but semver-breaking for near-zero gain.

**Verify** `cargo check --workspace --locked`.

---

### 18. Reconsider the `elasticsearch` feature

**Target files** 41 `cfg(feature = "elasticsearch")` sites, plus feature declarations in
`Cargo.toml`, `zebra-chain/Cargo.toml:36-37`, `zebra-state/Cargo.toml:37-41,75-77`,
`zebrad/Cargo.toml:84-86`.

**What to do** Both `Cargo.toml` files label this "Experimental elasticsearch support".
It is off by default and, as far as static analysis can tell, not exercised in CI.

The cost is not the 41 gates — it is that the feature threads a conditional *parameter*
through core signatures. `FinalizedState::new` takes
`#[cfg(feature = "elasticsearch")] enable_elastic_db: bool`
(`finalized_state.rs:160`, :181), which forces this at ~25 call sites, most of them in
tests:

```rust
FinalizedState::new(&Config::ephemeral(), &network,
                    #[cfg(feature = "elasticsearch")] false)
```

That attribute-in-argument-position is genuinely hard to read and it is load-bearing
noise in files that have nothing to do with Elasticsearch.

**Two options, and this is a product decision, not a refactor decision:**

- **(a) Remove the feature.** ~250 lines plus the call-site noise. Deletes a shipped
  (if experimental) capability — needs Zebra team sign-off before any code is written.
  Per `CLAUDE.md` this is exactly the kind of change that gets a PR closed without prior
  discussion.
- **(b) Keep it, hide the seam.** Leave the feature, but replace the conditional
  parameter with an always-present config struct field that is simply ignored when the
  feature is off. Removes all ~25 `#[cfg(...)] false` call-site warts for ~40 lines net,
  with no capability change and no sign-off needed.

**Recommendation: (b).** It captures most of the readability win at a fraction of the
risk and needs no product decision. Raise (a) with the team separately; if they confirm
the feature is dead, it becomes a clean follow-up.

**Est. LOC delta** −40 (option b) / −250 (option a)

**Risk** Low for (b). High for (a) — removes user-visible functionality, and the task
brief forbids behaviour changes.

**Verify** `cargo check -p zebra-state --all-targets` both with and without
`--features elasticsearch`; `cargo test -p zebra-state`.

---

### 19. Sweep the remaining expanded Tower bounds

**Target** The ~15 `Error = zebra_*::BoxError` sites left after slice 7, across
`zebra-rpc/src`, `zebrad/src`, `zebra-consensus/src`.

**What to do** Same mechanical substitution as slice 7, applied to the long tail.
Includes `zebra-rpc/src/methods/types/get_block_template.rs` and the test helpers in
`zebra-rpc/src/methods/tests/snapshot.rs`.

Worth doing as one commit after slice 7 has proven the pattern, rather than bundling —
if slice 7 hits an unexpected bound mismatch, this slice inherits it.

**Est. LOC delta** −80

**Risk** Low.

**Verify** `cargo check --workspace --all-targets --locked`.

---

### 20. Consolidate `zebra-chain/src/error.rs`

**Target file** `zebra-chain/src/error.rs`

**What to do** The file opens with `// TODO: Move all these enums into a common enum at
the bottom.` After slice 4 removes `NoteCommitmentError` and `KeyError`, three remain:
`RandError`, `NoteError`, `AddressError`. Each has an
`InsufficientRandomness(#[from] RandError)` variant with an identical
`#[error("Randomness generation failure")]` string.

Do **not** execute the TODO as written — merging them into one enum would change error
types across the public API for no readability gain.

Instead: after slice 4, re-read the file and decide whether the remaining duplication is
worth touching at all. It may well not be. The actionable part is deleting the stale
TODO comment, which has outlived its plan.

Listed so the TODO gets resolved-or-removed rather than surviving another refactor pass.

**Est. LOC delta** −2

**Risk** Low.

**Verify** `cargo check -p zebra-chain`.

---

### 21. Re-run the dead-code scan after slices 1-8

**Target** Whole workspace.

**What to do** Not a code change — a repeat of the Method section's scans once the first
wave lands. Deletions cascade: `any_block_by_hash` only became dead because
`any_prev_block_hash_for_hash` was removed, and there will be more of those. One pass
found one cascade by hand; a re-run finds the rest mechanically.

Scan 2 (unused `pub` items) is the one to repeat — it takes seconds and its output is
directly actionable.

**Est. LOC delta** −150 (speculative)

**Risk** Low.

**Verify** Per-crate, as for slices 1-5.

---

### 22. Enable `dead_code` denial once the backlog is clear

**Target** `.cargo/config.toml` or per-crate `lib.rs` lint attributes.

**What to do** After slices 1-6, 14, and 21, the workspace should be close to
`dead_code`-clean. At that point the lint can be escalated so the backlog does not
rebuild.

Do not attempt this before the backlog is cleared — `-D dead_code` against 178 existing
sites is unactionable. And check how the existing `#[allow(missing_docs)]` module
attributes in `zebrad/src/components.rs` interact before changing workspace-wide lint
config.

This is the slice that makes the other 21 stick. Without it, the census regrows.

**Est. LOC delta** 0

**Risk** Low, but must be sequenced last.

**Verify** `cargo clippy --workspace --all-targets -- -D warnings`.

---

## Totals

| Confidence | Slices | Est. LOC removed |
| --- | --- | ---: |
| High — confirmed dead, compile-checked | 1-8, 12, 13, 15, 16 | ~1,150 |
| Medium — dedup, needs care | 9, 10, 11, 19 | ~880 |
| Speculative / needs a decision | 14, 17, 18, 20, 21 | ~300 |
| **Total** | | **~2,300** |

Against ~227,600 lines of Rust that is about 1%. The readability win is concentrated
rather than spread: slices 7, 9, 10, and 16 remove *repetition* from four of the files a
reader is most likely to open first, which matters more than the raw count.

## Suggested order

1. **Slices 1-6** — pure deletion, per-crate, independently revertable. Land these first
   and separately; they carry the most value per unit of review effort.
2. **Slices 7, 8, 12, 13, 15, 16** — small structural cleanups, still low-risk.
3. **Slice 21** — re-scan, then fold the cascade into another deletion commit.
4. **Slices 9, 10** — the two macro/generic consolidations. One PR each. These need real
   review attention, especially slice 9's byte-order handling.
5. **Slices 14, 19, 20** — the long tails.
6. **Slice 18** — only after the team weighs in on option (a) vs (b).
7. **Slice 22** — last, to hold the line.

## Before opening any PR

`CLAUDE.md` has a mandatory contribution gate: a linked issue **with a Zebra team
member's acknowledgment**, predating the PR. It also lists "refactors or improvements
nobody asked for" as an explicit closure reason — which describes this entire plan
absent that discussion. Confirm maintainer status
(`gh api repos/ZcashFoundation/zebra --jq '.permissions.maintain'`) or get the issue
acknowledged before any of this becomes a PR.

Also required per repo policy: conventional-commit titles *and* branch commits, `!` on
anything semver-breaking (slices 1-8 and 17, see the caveat above), a `changie` fragment
per affected crate for user-visible changes, no `Co-Authored-By` or "generated with"
footers on any commit, and AI-use disclosure in the PR description.
