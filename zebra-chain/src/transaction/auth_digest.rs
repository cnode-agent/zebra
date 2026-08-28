//! Authorizing digests for Zcash transactions.

use std::array::TryFromSliceError;

use crate::serialization::{
    impl_hex_display, BytesInDisplayOrder, ReadZcashExt, SerializationError, WriteZcashExt,
    ZcashDeserialize, ZcashSerialize,
};

#[cfg(any(test, feature = "proptest-impl"))]
use proptest_derive::Arbitrary;

/// An authorizing data commitment hash as specified in [ZIP-244].
///
/// Note: Zebra displays transaction and block hashes in big-endian byte-order,
/// following the u256 convention set by Bitcoin and zcashd.
///
/// [ZIP-244]: https://zips.z.cash/zip-0244
#[derive(Copy, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
#[cfg_attr(any(test, feature = "proptest-impl"), derive(Arbitrary))]
pub struct AuthDigest(pub [u8; 32]);

impl BytesInDisplayOrder<true> for AuthDigest {
    fn bytes_in_serialized_order(&self) -> [u8; 32] {
        self.0
    }

    fn from_bytes_in_serialized_order(bytes: [u8; 32]) -> Self {
        AuthDigest(bytes)
    }
}

// From<&Transaction> for AuthDigest is defined in transaction.rs (TryFrom)

impl From<[u8; 32]> for AuthDigest {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl TryFrom<&[u8]> for AuthDigest {
    type Error = TryFromSliceError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Ok(AuthDigest(bytes.try_into()?))
    }
}

impl From<AuthDigest> for [u8; 32] {
    fn from(auth_digest: AuthDigest) -> Self {
        auth_digest.0
    }
}

impl From<&AuthDigest> for [u8; 32] {
    fn from(auth_digest: &AuthDigest) -> Self {
        (*auth_digest).into()
    }
}

impl_hex_display!(ToHex for AuthDigest, bytes_in_display_order);
impl_hex_display!(FromHex for AuthDigest, reverse_then_into: 32);
impl_hex_display!(Display for AuthDigest);
impl_hex_display!(Debug for AuthDigest, "AuthDigest");

impl std::str::FromStr for AuthDigest {
    type Err = SerializationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0; 32];
        if hex::decode_to_slice(s, &mut bytes[..]).is_err() {
            Err(SerializationError::Parse("hex decoding error"))
        } else {
            bytes.reverse();
            Ok(AuthDigest(bytes))
        }
    }
}

impl ZcashSerialize for AuthDigest {
    fn zcash_serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), std::io::Error> {
        writer.write_32_bytes(&self.into())
    }
}

impl ZcashDeserialize for AuthDigest {
    fn zcash_deserialize<R: std::io::Read>(mut reader: R) -> Result<Self, SerializationError> {
        Ok(reader.read_32_bytes()?.into())
    }
}
