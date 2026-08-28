//! Generic async batch verifier service for signature schemes.
//!
//! The Ed25519, RedJubjub and RedPallas verifiers only differ in the batch verifier they
//! wrap, the item they queue, and the names they report. [`Scheme`] carries those
//! differences, and [`Verifier`] provides the batching, threadpool, channel and metrics
//! machinery they share.

use std::{
    fmt,
    future::Future,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::watch;
use tower::{util::ServiceFn, Service};
use tower_batch_control::{Batch, BatchControl, RequestWeight};
use tower_fallback::Fallback;

use crate::BoxError;

use super::{spawn_fifo, spawn_fifo_and_convert};

/// A signature scheme which can verify a batch of signatures at once.
///
/// Implementors are unit types used purely as a type-level tag: [`Verifier`] and [`Item`]
/// are parameterised over them, but never hold a value of one.
pub trait Scheme: Copy + fmt::Debug + Send + Sync + 'static {
    /// The scheme's batch verifier, which accumulates signatures until it is verified.
    type BatchVerifier: Default + Send + 'static;

    /// A single signature queued in [`Self::BatchVerifier`].
    type BatchItem: Clone + fmt::Debug + Send + 'static;

    /// The error returned when verification fails.
    ///
    /// `Copy` is required because each batch result is broadcast to every item in the
    /// batch through a [`watch`] channel, which only hands out borrows.
    type Error: std::error::Error + Copy + Into<BoxError> + Send + Sync + 'static;

    /// The scheme's name, used in trace messages and as the batch duration metric's
    /// `verifier` label.
    const NAME: &'static str;

    /// The name of the counter incremented once per valid signature.
    const VALIDATED_METRIC: &'static str;

    /// The name of the counter incremented once per invalid signature.
    const INVALID_METRIC: &'static str;

    /// Adds `item` to `batch`.
    fn queue(batch: &mut Self::BatchVerifier, item: Self::BatchItem);

    /// Verifies every signature in `batch` at once.
    fn verify_batch(batch: Self::BatchVerifier) -> Result<(), Self::Error>;

    /// Verifies a single signature on its own.
    fn verify_single(item: Self::BatchItem) -> Result<(), Self::Error>;
}

/// The type of verification results.
type VerifyResult<S> = Result<(), <S as Scheme>::Error>;

/// The type of the batch sender channel.
type Sender<S> = watch::Sender<Option<VerifyResult<S>>>;

/// The type of the batch item.
/// This is a newtype around the scheme's own batch item.
#[derive(Clone, Debug)]
pub struct Item<S: Scheme>(S::BatchItem);

impl<S: Scheme> RequestWeight for Item<S> {}

impl<S: Scheme> Item<S> {
    /// Wraps the scheme's own batch item.
    pub(super) fn new(item: S::BatchItem) -> Self {
        Self(item)
    }

    /// Returns the scheme's own batch item.
    pub(super) fn into_inner(self) -> S::BatchItem {
        self.0
    }

    /// Verifies this signature on its own.
    pub(super) fn verify_single(self) -> VerifyResult<S> {
        S::verify_single(self.0)
    }
}

/// The type of the global batch verification context for scheme `S`.
pub type VerifierService<S> = Fallback<
    Batch<Verifier<S>, Item<S>>,
    ServiceFn<fn(Item<S>) -> BoxFuture<'static, Result<(), BoxError>>>,
>;

/// Creates the global batch verification context for scheme `S`.
///
/// This service transparently batches contemporaneous signature verifications,
/// handling batch failures by falling back to individual verification.
pub(super) fn verifier_service<S: Scheme>() -> VerifierService<S> {
    Fallback::new(
        Batch::new(
            Verifier::<S>::default(),
            super::MAX_BATCH_SIZE,
            None,
            super::MAX_BATCH_LATENCY,
        ),
        // We want to fallback to individual verification if batch verification fails,
        // so we need a Service to use.
        //
        // Because we have to specify the type of a static, we need to be able to
        // write the type of the closure and its return value. But both closures and
        // async blocks have unnameable types. So instead we cast the closure to a function
        // (which is possible because it doesn't capture any state), and use a BoxFuture
        // to erase the result type.
        // (We can't use BoxCloneService to erase the service type, because it is !Sync.)
        tower::service_fn(
            (|item: Item<S>| Verifier::<S>::verify_single_spawning(item).boxed()) as fn(_) -> _,
        ),
    )
}

/// A signature verifier service for scheme `S`.
pub struct Verifier<S: Scheme> {
    /// A batch verifier for `S` signatures.
    batch: S::BatchVerifier,

    /// A channel for broadcasting the result of a batch to the futures for each batch item.
    ///
    /// Each batch gets a newly created channel, so there is only ever one result sent per channel.
    /// Tokio doesn't have a oneshot multi-consumer channel, so we use a watch channel.
    tx: Sender<S>,
}

impl<S: Scheme> Default for Verifier<S> {
    fn default() -> Self {
        let batch = S::BatchVerifier::default();
        let (tx, _) = watch::channel(None);
        Self { batch, tx }
    }
}

impl<S: Scheme> Verifier<S> {
    /// Returns the batch verifier and channel sender from `self`,
    /// replacing them with a new empty batch.
    fn take(&mut self) -> (S::BatchVerifier, Sender<S>) {
        // Use a new verifier and channel for each batch.
        let batch = mem::take(&mut self.batch);

        let (tx, _) = watch::channel(None);
        let tx = mem::replace(&mut self.tx, tx);

        (batch, tx)
    }

    /// Synchronously process the batch, and send the result using the channel sender.
    /// This function blocks until the batch is completed.
    fn verify(batch: S::BatchVerifier, tx: Sender<S>) {
        let result = S::verify_batch(batch);
        let _ = tx.send(Some(result));
    }

    /// Flush the batch using a thread pool, and return the result via the channel.
    /// This returns immediately, usually before the batch is completed.
    fn flush_blocking(&mut self) {
        let (batch, tx) = self.take();

        // Correctness: Do CPU-intensive work on a dedicated thread, to avoid blocking other futures.
        //
        // We don't care about execution order here, because this method is only called on drop.
        tokio::task::block_in_place(|| rayon::spawn_fifo(|| Self::verify(batch, tx)));
    }

    /// Flush the batch using a thread pool, and return the result via the channel.
    /// This function returns a future that becomes ready when the batch is completed.
    async fn flush_spawning(batch: S::BatchVerifier, tx: Sender<S>) {
        // Correctness: Do CPU-intensive work on a dedicated thread, to avoid blocking other futures.
        let start = std::time::Instant::now();
        let result = spawn_fifo(move || S::verify_batch(batch)).await;
        let duration = start.elapsed().as_secs_f64();

        let result_label = match &result {
            Ok(Ok(())) => "success",
            _ => "failure",
        };
        metrics::histogram!(
            "zebra.consensus.batch.duration_seconds",
            "verifier" => S::NAME,
            "result" => result_label
        )
        .record(duration);

        let _ = tx.send(result.ok());
    }

    /// Verify a single item using a thread pool, and return the result.
    async fn verify_single_spawning(item: Item<S>) -> Result<(), BoxError> {
        // Correctness: Do CPU-intensive work on a dedicated thread, to avoid blocking other futures.
        spawn_fifo_and_convert(move || item.verify_single()).await
    }
}

impl<S: Scheme> Service<BatchControl<Item<S>>> for Verifier<S> {
    type Response = ();
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: BatchControl<Item<S>>) -> Self::Future {
        match req {
            BatchControl::Item(item) => {
                tracing::trace!("got {} item", S::NAME);
                S::queue(&mut self.batch, item.into_inner());
                let mut rx = self.tx.subscribe();

                Box::pin(async move {
                    match rx.changed().await {
                        Ok(()) => {
                            // We use a new channel for each batch,
                            // so we always get the correct batch result here.
                            let result = rx.borrow()
                                .ok_or("threadpool unexpectedly dropped response channel sender. Is Zebra shutting down?")?;

                            if result.is_ok() {
                                tracing::trace!(?result, "validated {} signature", S::NAME);
                                metrics::counter!(S::VALIDATED_METRIC).increment(1);
                            } else {
                                tracing::trace!(?result, "invalid {} signature", S::NAME);
                                metrics::counter!(S::INVALID_METRIC).increment(1);
                            }

                            result.map_err(Into::into)
                        }
                        Err(_recv_error) => {
                            panic!("{} verifier was dropped without flushing", S::NAME)
                        }
                    }
                })
            }

            BatchControl::Flush => {
                tracing::trace!("got {} flush command", S::NAME);

                let (batch, tx) = self.take();

                Box::pin(Self::flush_spawning(batch, tx).map(Ok))
            }
        }
    }
}

impl<S: Scheme> Drop for Verifier<S> {
    fn drop(&mut self) {
        // We need to flush the current batch in case there are still any pending futures.
        // This returns immediately, usually before the batch is completed.
        self.flush_blocking();
    }
}
