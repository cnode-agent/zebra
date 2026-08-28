//! Async RedJubjub batch verifier service

use once_cell::sync::Lazy;
use rand::thread_rng;

use zebra_chain::primitives::redjubjub::*;

use super::signature_verifier::{self, Scheme};

#[cfg(test)]
mod tests;

/// The RedJubjub signature scheme.
#[derive(Copy, Clone, Debug)]
pub struct RedJubjub;

impl Scheme for RedJubjub {
    type BatchVerifier = batch::Verifier;
    type BatchItem = batch::Item;
    type Error = Error;

    const NAME: &'static str = "redjubjub";
    const VALIDATED_METRIC: &'static str = "signatures.redjubjub.validated";
    const INVALID_METRIC: &'static str = "signatures.redjubjub.invalid";

    fn queue(batch: &mut Self::BatchVerifier, item: Self::BatchItem) {
        batch.queue(item);
    }

    fn verify_batch(batch: Self::BatchVerifier) -> Result<(), Self::Error> {
        batch.verify(thread_rng())
    }

    fn verify_single(item: Self::BatchItem) -> Result<(), Self::Error> {
        item.verify_single()
    }
}

/// RedJubjub signature verifier service
pub type Verifier = signature_verifier::Verifier<RedJubjub>;

/// The type of the batch item.
/// This is a newtype around a `RedJubjubItem`.
pub type Item = signature_verifier::Item<RedJubjub>;

impl<T: Into<batch::Item>> From<T> for Item {
    fn from(value: T) -> Self {
        Self::new(value.into())
    }
}

/// Global batch verification context for RedJubjub signatures.
///
/// This service transparently batches contemporaneous signature verifications,
/// handling batch failures by falling back to individual verification.
///
/// Note that making a `Service` call requires mutable access to the service, so
/// you should call `.clone()` on the global handle to create a local, mutable
/// handle.
pub static VERIFIER: Lazy<signature_verifier::VerifierService<RedJubjub>> =
    Lazy::new(signature_verifier::verifier_service);
