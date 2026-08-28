//! A AddressBookPeers trait for getting the [`MetaAddr`] of recently live peers.

use chrono::Utc;

use crate::{meta_addr::MetaAddr, PeerSocketAddr};

#[cfg(any(test, feature = "proptest-impl"))]
pub mod mock;

#[cfg(any(test, feature = "proptest-impl"))]
pub use mock::MockAddressBookPeers;

/// Method signatures for getting [`MetaAddr`]s of recently live peers.
pub trait AddressBookPeers {
    /// Return an Vec of peers we've seen recently, in reconnection attempt order.
    fn recently_live_peers(&self, now: chrono::DateTime<Utc>) -> Vec<MetaAddr>;

    /// Add a peer to the address book.
    fn add_peer(&mut self, peer: PeerSocketAddr) -> bool;
}

/// Trait alias for [`AddressBookPeers`] handles that can be shared between async tasks.
///
/// It adds the common bounds that every service holding an address book handle requires.
pub trait AddressBookService: AddressBookPeers + Clone + Send + Sync + 'static {}

impl<T> AddressBookService for T where T: AddressBookPeers + Clone + Send + Sync + 'static {}
