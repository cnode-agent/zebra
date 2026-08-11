//! Wrapper around [`zcash_tachyon::TachyonBundle`] that adds the trait impls
//! Zebra needs (`PartialEq`, `Eq`, optionally `serde`).
//!
//! Upstream tachyon does not derive these on `TachyonBundle`, and orphan
//! rules forbid us from implementing them on the foreign type directly. This
//! newtype lets us add them locally.
//!
//! `PartialEq` is implemented via byte comparison through tachyon's own
//! consensus serializer, which is canonical for any constructible bundle.

use zcash_tachyon::TachyonBundle;

/// The tachyon shielded data in a V7 transaction: a [`TachyonBundle`] in one of its wire states
/// (proof-stamped or pointer-stamped; `NoBundle` is represented as `None` at the transaction
/// level).
#[derive(Clone, Debug)]
pub struct TachyonShieldedData(pub TachyonBundle);

impl TachyonShieldedData {
    /// The bundle's actions, regardless of stamp state.
    ///
    /// Empty for `NoBundle`, which Zebra never stores here — `serialize.rs` maps it to a `None`
    /// `tachyon_shielded_data` field instead.
    pub fn actions(&self) -> &[zcash_tachyon::Action] {
        match &self.0 {
            TachyonBundle::NoBundle => &[],
            TachyonBundle::Proven(bundle) => &bundle.actions,
            TachyonBundle::Adjunct(bundle) => &bundle.actions,
        }
    }

    /// The bundle's value balance (zero for `NoBundle`).
    pub fn value_balance(&self) -> zcash_tachyon::value::Balance {
        match &self.0 {
            TachyonBundle::NoBundle => zcash_tachyon::value::Balance::ZERO,
            TachyonBundle::Proven(bundle) => bundle.value_balance,
            TachyonBundle::Adjunct(bundle) => bundle.value_balance,
        }
    }
}

impl From<TachyonBundle> for TachyonShieldedData {
    fn from(bundle: TachyonBundle) -> Self {
        Self(bundle)
    }
}

impl PartialEq for TachyonShieldedData {
    fn eq(&self, other: &Self) -> bool {
        let mut a = Vec::new();
        let mut b = Vec::new();
        self.0.write(&mut a).expect("write to Vec is infallible");
        other.0.write(&mut b).expect("write to Vec is infallible");
        a == b
    }
}

impl Eq for TachyonShieldedData {}

#[cfg(any(test, feature = "proptest-impl"))]
pub mod mock {
    //! Mock tachyon bundles for tests in this crate and its reverse dependencies.
    //!
    //! The bundles here carry a genuine (mock proof system) stamp rather than a hand-built one,
    //! so their `hStampActionsTachyon` is the real covered-actions digest of their actions. That
    //! makes them usable by code that tells the bundle shapes apart, like block production.
    //!
    //! Signatures are only valid over the placeholder sighash they are signed with, so these
    //! bundles are not usable by code that verifies signatures.

    use zcash_tachyon::{
        action, bundle,
        entropy::ActionEntropy,
        keys::private,
        note::{CommitmentTrapdoor, Note, NullifierTrapdoor},
        value, Anchor, PointerStamp, ProofStamp, TachyonBundle,
    };

    use super::TachyonShieldedData;

    /// A proof-stamped bundle over one output action, with a genuine stamp.
    fn proven_bundle() -> zcash_tachyon::Bundle<ProofStamp> {
        let mut rng = rand::thread_rng();

        let sk = private::SpendingKey::random(&mut rng);
        let note = Note {
            pk: sk.derive_payment_key(),
            value: value::Positive::try_from(100u64)
                .expect("value is positive and below MAX_MONEY"),
            psi: NullifierTrapdoor::random(&mut rng),
            rcm: CommitmentTrapdoor::random(&mut rng),
        };
        let output_plan = action::Plan::output(
            note,
            ActionEntropy::random(&mut rng),
            value::Trapdoor::random(&mut rng),
        );
        let plan = bundle::Plan::new(vec![], vec![output_plan]);

        let stamp = plan
            .stamp_plan(Anchor::default())
            .prove(&mut rng, &sk.derive_proof_private(), vec![])
            .expect("an output-only stamp plan proves under the mock proof system");

        plan.sign(&mut rng, &[0u8; 32], &sk.derive_auth_private())
            .expect("bundle plan has matching signatures and an in-range value balance")
            .stamp(stamp)
    }

    /// An *autonome*: a proof stamp covering exactly its own actions, which a block can include
    /// on its own.
    ///
    /// `tachygrams` replaces the stamp's own tachygrams when it is non-empty, so callers can
    /// build transactions that deliberately reveal the same tachygram. The replacement doesn't
    /// affect the covered-actions digest.
    pub fn autonome(tachygrams: &[u64]) -> TachyonShieldedData {
        let mut bundle = proven_bundle();

        if !tachygrams.is_empty() {
            bundle.stamp.tachygrams = tachygrams
                .iter()
                .map(|&tachygram| {
                    zcash_tachyon::Tachygram::from(halo2::pasta::pallas::Base::from(tachygram))
                })
                .collect();
        }

        TachyonShieldedData(TachyonBundle::Proven(bundle))
    }

    /// An *aggregate*: a proof stamp covering actions that live in pointer-stamped transactions
    /// elsewhere in the block, so it is only valid alongside all of them.
    pub fn aggregate() -> TachyonShieldedData {
        let mut bundle = proven_bundle();

        // A digest that isn't the one over this bundle's own actions is exactly what an
        // aggregate carries: the digest over a wider action set.
        bundle.stamp.coverage = [0u8; 32];

        TachyonShieldedData(TachyonBundle::Proven(bundle))
    }

    /// An *adjunct*: a bundle whose stamp a miner stripped, leaving a pointer to the aggregate
    /// covering its actions.
    pub fn adjunct(aggregate_wtxid: [u8; 64]) -> TachyonShieldedData {
        TachyonShieldedData(TachyonBundle::Adjunct(proven_bundle().strip(
            PointerStamp::try_from(aggregate_wtxid).expect("aggregate wtxid is nonzero"),
        )))
    }
}

#[cfg(any(test, feature = "proptest-impl", feature = "elasticsearch"))]
impl serde::Serialize for TachyonShieldedData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let mut buf = Vec::new();
        self.0.write(&mut buf).map_err(S::Error::custom)?;
        serde::Serialize::serialize(&buf, serializer)
    }
}

#[cfg(any(test, feature = "proptest-impl", feature = "elasticsearch"))]
impl<'de> serde::Deserialize<'de> for TachyonShieldedData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let bytes = <Vec<u8> as serde::Deserialize>::deserialize(deserializer)?;
        match TachyonBundle::read(&bytes[..]).map_err(D::Error::custom)? {
            TachyonBundle::NoBundle => Err(D::Error::custom(
                "tachyon bundle absent in wrapped serialization",
            )),
            bundle => Ok(Self(bundle)),
        }
    }
}
