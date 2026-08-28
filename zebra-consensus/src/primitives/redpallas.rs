//! Async RedPallas batch verifier service

use once_cell::sync::Lazy;
use rand::thread_rng;

use zebra_chain::primitives::reddsa::{batch, orchard, Error, Signature, VerificationKeyBytes};

use super::signature_verifier::{self, Scheme};

#[cfg(test)]
mod tests;

/// The RedPallas signature scheme.
#[derive(Copy, Clone, Debug)]
pub struct RedPallas;

impl Scheme for RedPallas {
    type BatchVerifier = batch::Verifier<orchard::SpendAuth, orchard::Binding>;
    type BatchItem = batch::Item<orchard::SpendAuth, orchard::Binding>;
    type Error = Error;

    const NAME: &'static str = "redpallas";
    const VALIDATED_METRIC: &'static str = "signatures.redpallas.validated";
    const INVALID_METRIC: &'static str = "signatures.redpallas.invalid";

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

/// RedPallas signature verifier service
pub type Verifier = signature_verifier::Verifier<RedPallas>;

/// The type of the batch item.
/// This is a newtype around a `RedPallasItem`.
pub type Item = signature_verifier::Item<RedPallas>;

impl Item {
    /// Create a batch item from a `SpendAuth` signature.
    pub fn from_spendauth(
        vk_bytes: VerificationKeyBytes<orchard::SpendAuth>,
        sig: Signature<orchard::SpendAuth>,
        msg: &impl AsRef<[u8]>,
    ) -> Self {
        Self::new(batch::Item::from_spendauth(vk_bytes, sig, msg))
    }

    /// Create a batch item from a `Binding` signature.
    pub fn from_binding(
        vk_bytes: VerificationKeyBytes<orchard::Binding>,
        sig: Signature<orchard::Binding>,
        msg: &impl AsRef<[u8]>,
    ) -> Self {
        Self::new(batch::Item::from_binding(vk_bytes, sig, msg))
    }
}

impl From<Item> for batch::Item<orchard::SpendAuth, orchard::Binding> {
    fn from(item: Item) -> Self {
        item.into_inner()
    }
}

/// Global batch verification context for RedPallas signatures.
///
/// This service transparently batches contemporaneous signature verifications,
/// handling batch failures by falling back to individual verification.
///
/// Note that making a `Service` call requires mutable access to the service, so
/// you should call `.clone()` on the global handle to create a local, mutable
/// handle.
pub static VERIFIER: Lazy<signature_verifier::VerifierService<RedPallas>> =
    Lazy::new(signature_verifier::verifier_service);
