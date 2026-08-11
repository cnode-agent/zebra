//! Tests for ZIP-317 transaction selection for block template production

#![allow(clippy::unwrap_in_result)]

use std::sync::Arc;

use zcash_keys::address::Address;
use zcash_transparent::address::TransparentAddress;

use zebra_chain::{
    amount::Amount,
    block::{Header, Height, MAX_BLOCK_BYTES},
    parameters::{Network, NetworkUpgrade},
    transaction::{
        self, tachyon_mock, LockTime, TachyonShieldedData, Transaction, UnminedTx,
        VerifiedUnminedTx,
    },
    transparent::OutPoint,
};
use zebra_node_services::mempool::TransactionDependencies;

use crate::methods::types::{get_block_template::MinerParams, transaction::TransactionTemplate};

use super::{max_transaction_count_size, select_mempool_transactions};

/// The miner parameters used by these tests.
fn miner_params() -> MinerParams {
    MinerParams::from(Address::from(TransparentAddress::PublicKeyHash([0x7e; 20])))
}

/// A verified mempool transaction carrying only `tachyon_shielded_data`.
///
/// Its miner fee is well above the conventional fee, so it is a conventional-fee candidate with
/// no unpaid actions.
fn tachyon_tx(tachyon_shielded_data: TachyonShieldedData) -> VerifiedUnminedTx {
    let tx = Transaction::V7 {
        network_upgrade: NetworkUpgrade::Nu7,
        lock_time: LockTime::min_lock_time_timestamp(),
        expiry_height: Height(0),
        inputs: Vec::new(),
        outputs: Vec::new(),
        sapling_shielded_data: None,
        orchard_shielded_data: None,
        ironwood_shielded_data: None,
        tachyon_shielded_data: Some(tachyon_shielded_data),
    };

    VerifiedUnminedTx::new(
        UnminedTx::from(tx),
        Amount::try_from(1_000_000).expect("valid amount"),
        0,
        0,
        Arc::new(Vec::new()),
    )
    .expect("mock tachyon transaction passes the ZIP-317 mempool checks")
}

#[test]
fn excludes_tx_with_unselected_dependencies() {
    let network = Network::Mainnet;
    let mut mempool_tx_deps = TransactionDependencies::default();

    let unmined_tx = network
        .unmined_transactions_in_blocks(..)
        .next()
        .expect("should not be empty");

    mempool_tx_deps.add(
        unmined_tx.transaction.id.mined_id(),
        vec![OutPoint::from_usize(transaction::Hash([0; 32]), 0)],
    );

    assert_eq!(
        select_mempool_transactions(
            &network,
            Height(1_000_000),
            &MinerParams::from(Address::from(TransparentAddress::PublicKeyHash([0x7e; 20]))),
            vec![unmined_tx],
            mempool_tx_deps,
            None,
        ),
        vec![],
        "should not select any transactions when dependencies are unavailable"
    );
}

#[test]
fn includes_tx_with_selected_dependencies() {
    let network = Network::Mainnet;
    let unmined_txs: Vec<_> = network.unmined_transactions_in_blocks(..).take(3).collect();

    let dependent_tx1 = unmined_txs.first().expect("should have 3 txns");
    let dependent_tx2 = unmined_txs.get(1).expect("should have 3 txns");
    let independent_tx_id = unmined_txs
        .get(2)
        .expect("should have 3 txns")
        .transaction
        .id
        .mined_id();

    let mut mempool_tx_deps = TransactionDependencies::default();
    mempool_tx_deps.add(
        dependent_tx1.transaction.id.mined_id(),
        vec![OutPoint::from_usize(independent_tx_id, 0)],
    );
    mempool_tx_deps.add(
        dependent_tx2.transaction.id.mined_id(),
        vec![
            OutPoint::from_usize(independent_tx_id, 0),
            OutPoint::from_usize(transaction::Hash([0; 32]), 0),
        ],
    );

    let selected_txs = select_mempool_transactions(
        &network,
        Height(1_000_000),
        &MinerParams::from(Address::from(TransparentAddress::PublicKeyHash([0x7e; 20]))),
        unmined_txs.clone(),
        mempool_tx_deps.clone(),
        None,
    );

    assert_eq!(
        selected_txs.len(),
        2,
        "should select the independent transaction and 1 of the dependent txs, selected: {selected_txs:?}"
    );

    let selected_tx_by_id = |id| {
        selected_txs
            .iter()
            .find(|(_, tx)| tx.transaction.id.mined_id() == id)
    };

    let (dependency_depth, _) =
        selected_tx_by_id(independent_tx_id).expect("should select the independent tx");

    assert_eq!(
        *dependency_depth, 0,
        "should return a dependency depth of 0 for the independent tx"
    );

    let (dependency_depth, _) = selected_tx_by_id(dependent_tx1.transaction.id.mined_id())
        .expect("should select dependent_tx1");

    assert_eq!(
        *dependency_depth, 1,
        "should return a dependency depth of 1 for the dependent tx"
    );
}

/// Checks that transaction selection reserves space for the block header and the transaction
/// count, which [`MAX_BLOCK_BYTES`] covers: a transaction exactly filling the remaining safe
/// budget is selected, and a transaction one byte larger is not (GHSA-95m2-vx53-v2jw).
#[test]
fn reserves_space_for_block_header_and_transaction_count() {
    let network = Network::Mainnet;
    let height = Height(1_000_000);
    let miner_params =
        MinerParams::from(Address::from(TransparentAddress::PublicKeyHash([0x7e; 20])));

    let coinbase_tx_size =
        TransactionTemplate::new_coinbase(&network, height, &miner_params, Amount::zero())
            .expect("valid coinbase transaction template")
            .data
            .as_ref()
            .len();

    let safe_budget = usize::try_from(MAX_BLOCK_BYTES).expect("fits in memory")
        - Header::serialized_size(&network)
        - max_transaction_count_size()
        - coinbase_tx_size;

    let mut unmined_tx = network
        .unmined_transactions_in_blocks(..)
        .next()
        .expect("should not be empty");

    unmined_tx.transaction.size = safe_budget;

    assert_eq!(
        select_mempool_transactions(
            &network,
            height,
            &miner_params,
            vec![unmined_tx.clone()],
            TransactionDependencies::default(),
            None,
        )
        .len(),
        1,
        "should select a transaction exactly filling the safe block budget"
    );

    unmined_tx.transaction.size = safe_budget + 1;

    assert_eq!(
        select_mempool_transactions(
            &network,
            height,
            &miner_params,
            vec![unmined_tx],
            TransactionDependencies::default(),
            None,
        ),
        vec![],
        "should not select a transaction one byte over the safe block budget"
    );
}

/// Only tachyon autonomes can be mined without transactions Zebra can't produce, so aggregates
/// and adjuncts are never selected.
#[test]
fn excludes_unminable_tachyon_transactions() {
    let network = Network::Mainnet;
    let height = Height(1_000_000);

    let autonome = tachyon_tx(tachyon_mock::autonome(&[]));
    let autonome_id = autonome.transaction.id.mined_id();

    let selected_txs = select_mempool_transactions(
        &network,
        height,
        &miner_params(),
        vec![
            autonome,
            // Its stamp covers actions carried by adjuncts Zebra can't assemble.
            tachyon_tx(tachyon_mock::aggregate()),
            // Its actions are proven by an aggregate that isn't in the mempool.
            tachyon_tx(tachyon_mock::adjunct([0xEE; 64])),
        ],
        TransactionDependencies::default(),
        None,
    );

    assert_eq!(
        selected_txs
            .iter()
            .map(|(_, tx)| tx.transaction.id.mined_id())
            .collect::<Vec<_>>(),
        vec![autonome_id],
        "should only select the autonome",
    );
}

/// All tachygrams in a block must be distinct, so at most one of two transactions revealing the
/// same tachygram is selected.
#[test]
fn excludes_duplicate_tachygrams() {
    let network = Network::Mainnet;
    let height = Height(1_000_000);

    let selected_txs = select_mempool_transactions(
        &network,
        height,
        &miner_params(),
        vec![
            tachyon_tx(tachyon_mock::autonome(&[7, 8])),
            tachyon_tx(tachyon_mock::autonome(&[7, 9])),
        ],
        TransactionDependencies::default(),
        None,
    );

    assert_eq!(
        selected_txs.len(),
        1,
        "should select only one of two transactions revealing the same tachygram, \
         selected: {selected_txs:?}",
    );

    // Distinct tachygrams don't conflict.
    let selected_txs = select_mempool_transactions(
        &network,
        height,
        &miner_params(),
        vec![
            tachyon_tx(tachyon_mock::autonome(&[7, 8])),
            tachyon_tx(tachyon_mock::autonome(&[9, 10])),
        ],
        TransactionDependencies::default(),
        None,
    );

    assert_eq!(
        selected_txs.len(),
        2,
        "should select both transactions when their tachygrams are distinct, \
         selected: {selected_txs:?}",
    );
}
