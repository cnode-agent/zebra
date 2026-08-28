//! Async Ed25519 batch verifier service

use once_cell::sync::Lazy;
use rand::thread_rng;

use zebra_chain::primitives::ed25519::*;

use super::signature_verifier::{self, Scheme};

#[cfg(test)]
mod tests;

/// The Ed25519 signature scheme.
#[derive(Copy, Clone, Debug)]
pub struct Ed25519;

impl Scheme for Ed25519 {
    type BatchVerifier = batch::Verifier;
    type BatchItem = batch::Item;
    type Error = Error;

    const NAME: &'static str = "ed25519";
    const VALIDATED_METRIC: &'static str = "signatures.ed25519.validated";
    const INVALID_METRIC: &'static str = "signatures.ed25519.invalid";

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

/// Ed25519 signature verifier service
pub type Verifier = signature_verifier::Verifier<Ed25519>;

/// The type of the batch item.
/// This is a newtype around an `Ed25519Item`.
pub type Item = signature_verifier::Item<Ed25519>;

impl<'msg, M: AsRef<[u8]> + ?Sized> From<(VerificationKeyBytes, Signature, &'msg M)> for Item {
    fn from(tup: (VerificationKeyBytes, Signature, &'msg M)) -> Self {
        Self::new(batch::Item::from(tup))
    }
}

impl From<Item> for batch::Item {
    fn from(item: Item) -> Self {
        item.into_inner()
    }
}

/// Global batch verification context for Ed25519 signatures.
///
/// This service transparently batches contemporaneous signature verifications,
/// handling batch failures by falling back to individual verification.
///
/// Note that making a `Service` call requires mutable access to the service, so
/// you should call `.clone()` on the global handle to create a local, mutable
/// handle.
pub static VERIFIER: Lazy<signature_verifier::VerifierService<Ed25519>> =
    Lazy::new(signature_verifier::verifier_service);
