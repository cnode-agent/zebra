//! Defines method signatures for checking if the synchronizer is likely close to the network chain tip.

#[cfg(any(test, feature = "proptest-impl"))]
pub mod mock;

#[cfg(any(test, feature = "proptest-impl"))]
pub use mock::MockSyncStatus;

/// An interface for checking if the synchronization is likely close to the network chain tip.
pub trait ChainSyncStatus {
    /// Check if the synchronization is likely close to the network chain tip.
    fn is_close_to_tip(&self) -> bool;
}

/// Trait alias for [`ChainSyncStatus`] handles that can be shared between async tasks.
///
/// It adds the common bounds that every service holding a sync status handle requires.
pub trait ChainSyncStatusService: ChainSyncStatus + Clone + Send + Sync + 'static {}

impl<T> ChainSyncStatusService for T where T: ChainSyncStatus + Clone + Send + Sync + 'static {}
