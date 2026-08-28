//! A macro for the hex encoding, decoding and formatting boilerplate that Zebra's
//! byte-array-backed types repeat verbatim.

/// Implements the [`hex`] and [`std::fmt`] boilerplate shared by Zebra's
/// byte-array-backed types.
///
/// Each arm implements exactly one trait, so every invocation can be read against the
/// hand-written impl it replaces. Byte order is always spelled out at the call site:
/// there is no default, because getting it wrong silently reverses the hex shown on the
/// RPC surface instead of failing to compile.
///
/// # Arms
///
/// ```ignore
/// // `ToHex` for `&Type` and `Type`, hex-encoding `self.method()`:
/// impl_hex_display!(ToHex for Type, method);
/// // ... or an arbitrary expression, with `this` bound to a `&Type`:
/// impl_hex_display!(ToHex for Type, |this| <[u8; 32]>::from(this));
/// // `ToHex` for `&Type` only, for types that deliberately have no owned impl:
/// impl_hex_display!(ToHexRef for Type, method);
///
/// // `FromHex` via `BytesInDisplayOrder`, which reverses iff the type's impl says to:
/// impl_hex_display!(FromHex for Type, from_bytes_in_display_order: 32);
/// // `FromHex` that always reverses the decoded bytes, then uses `From<[u8; N]>`:
/// impl_hex_display!(FromHex for Type, reverse_then_into: 32);
///
/// // `Display` as the type's own display-order hex:
/// impl_hex_display!(Display for Type);
///
/// // `Debug` as `Type(<display-order hex>)`:
/// impl_hex_display!(Debug for Type, "Type");
/// // `Debug` as `Type(<hex of an arbitrary expression>)`, with `this` bound to `&Type`:
/// impl_hex_display!(Debug for Type, "Type", |this| this.0);
/// ```
macro_rules! impl_hex_display {
    // `ToHex` for both `&$type` and `$type`.
    (ToHex for $type:ty, |$this:ident| $bytes:expr) => {
        $crate::serialization::impl_hex_display!(ToHexRef for $type, |$this| $bytes);

        impl ::hex::ToHex for $type {
            fn encode_hex<T: FromIterator<char>>(&self) -> T {
                ::hex::ToHex::encode_hex(&self)
            }

            fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
                ::hex::ToHex::encode_hex_upper(&self)
            }
        }
    };

    (ToHex for $type:ty, $bytes:ident) => {
        $crate::serialization::impl_hex_display!(ToHex for $type, |this| this.$bytes());
    };

    // `ToHex` for `&$type` only.
    (ToHexRef for $type:ty, |$this:ident| $bytes:expr) => {
        impl ::hex::ToHex for &$type {
            fn encode_hex<T: FromIterator<char>>(&self) -> T {
                let $this: &$type = self;
                ::hex::ToHex::encode_hex(&$bytes)
            }

            fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
                let $this: &$type = self;
                ::hex::ToHex::encode_hex_upper(&$bytes)
            }
        }
    };

    (ToHexRef for $type:ty, $bytes:ident) => {
        $crate::serialization::impl_hex_display!(ToHexRef for $type, |this| this.$bytes());
    };

    // `FromHex` that hands the decoded big-endian bytes to
    // [`BytesInDisplayOrder::from_bytes_in_display_order`], which reverses them if and
    // only if the type's `BytesInDisplayOrder` impl asks for it.
    (FromHex for $type:ty, from_bytes_in_display_order: $len:literal) => {
        impl ::hex::FromHex for $type {
            type Error = <[u8; $len] as ::hex::FromHex>::Error;

            fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
                let bytes_in_display_order = <[u8; $len] as ::hex::FromHex>::from_hex(hex)?;

                Ok(Self::from_bytes_in_display_order(&bytes_in_display_order))
            }
        }
    };

    // `FromHex` that unconditionally reverses the decoded bytes, then builds the type
    // with its `From<[u8; $len]>` impl.
    (FromHex for $type:ty, reverse_then_into: $len:literal) => {
        impl ::hex::FromHex for $type {
            type Error = <[u8; $len] as ::hex::FromHex>::Error;

            fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
                let mut hash = <[u8; $len] as ::hex::FromHex>::from_hex(hex)?;
                hash.reverse();

                Ok(hash.into())
            }
        }
    };

    // `Display` as the type's own hex encoding.
    (Display for $type:ty) => {
        impl ::std::fmt::Display for $type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                f.write_str(&::hex::ToHex::encode_hex::<String>(&self))
            }
        }
    };

    // `Debug` as a one-field tuple holding the type's own hex encoding.
    (Debug for $type:ty, $label:literal) => {
        impl ::std::fmt::Debug for $type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                f.debug_tuple($label)
                    .field(&::hex::ToHex::encode_hex::<String>(&self))
                    .finish()
            }
        }
    };

    // `Debug` as a one-field tuple holding the hex of `$bytes`, which is *not*
    // necessarily the type's display order.
    (Debug for $type:ty, $label:literal, |$this:ident| $bytes:expr) => {
        impl ::std::fmt::Debug for $type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                let $this: &$type = self;

                f.debug_tuple($label)
                    .field(&::hex::encode($bytes))
                    .finish()
            }
        }
    };
}

pub(crate) use impl_hex_display;
