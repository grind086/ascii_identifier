//! Provides a stack-allocated ASCII string type.

use std::{
    fmt::{self, Display},
    hash::Hash,
    str::FromStr,
};

use nonmax::NonMaxU8;
use thiserror::Error;

/// A 16-byte [`AsciiIdentifier`] with space for up to 15 characters.
pub type AsciiIdentifier16 = AsciiIdentifier<15>;

/// A 24-byte [`AsciiIdentifier`] with space for up to 23 characters.
///
/// This type uses the same amount of stack space as a [`String`].
pub type AsciiIdentifier24 = AsciiIdentifier<23>;

/// A 32-byte [`AsciiIdentifier`] with space for up to 31 characters.
pub type AsciiIdentifier32 = AsciiIdentifier<31>;

/// A 64-byte [`AsciiIdentifier`] with space for up to 63 characters.
pub type AsciiIdentifier64 = AsciiIdentifier<63>;

/// A 128-byte [`AsciiIdentifier`] with space for up to 127 characters.
pub type AsciiIdentifier128 = AsciiIdentifier<127>;

/// A 256-byte [`AsciiIdentifier`] with space for up to 255 characters.
///
/// This is the maximum size of a constructable `AsciiIdentifier`.
pub type AsciiIdentifier256 = AsciiIdentifier<255>;

/// A stack-allocated ASCII string with at most `LEN` characters.
///
/// This type uses `LEN + 1` bytes, and is niche optimized.
///
/// # Panics
///
/// `LEN` can be at most `255`, however this cannot (currently) be guaranteed generically at
/// compile time. Attempting to construct an `AsciiIdentifier<LEN>` with `LEN > 255` will panic
/// unconditionally.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct AsciiIdentifier<const LEN: usize> {
    len: NonMaxU8,
    bytes: [u8; LEN],
}

impl<const LEN: usize> AsciiIdentifier<LEN> {
    /// Creates an empty `AsciiIdentifier`.
    pub const fn new() -> Self {
        assert_valid_len::<LEN>();
        Self {
            len: NonMaxU8::ZERO,
            bytes: [0; LEN],
        }
    }

    /// Creates a new `AsciiIdentifier` from the given ASCII string.
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let id = AsciiIdentifier::from_str("hello!").unwrap();
    /// assert_eq!(id, "hello!");
    /// ```
    ///
    /// # Errors
    ///
    /// An error will be returned if:
    /// - The string is longer than `LEN` bytes
    /// - The string is not valid ASCII
    pub const fn from_str(s: &str) -> Result<Self, AsciiIdentifierError> {
        Self::from_bytes(s.as_bytes())
    }

    /// Creates a new `AsciiIdentifier` from the given ASCII bytes.
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let id = AsciiIdentifier::from_bytes(&[0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x21]).unwrap();
    /// assert_eq!(id, "hello!");
    /// ```
    ///
    /// # Errors
    ///
    /// An error will be returned if:
    /// - The slice is longer than `LEN` bytes
    /// - The slice does not contain valid ASCII bytes
    pub const fn from_bytes(s: &[u8]) -> Result<Self, AsciiIdentifierError> {
        match validate_ascii_slice(LEN, s) {
            // SAFETY: The slice contains 1 to `LEN` ASCII bytes.
            Ok(()) => Ok(unsafe { Self::from_bytes_unchecked(s) }),
            Err(err) => Err(err),
        }
    }

    /// Constructs an `AsciiIdentifier` from the given bytes, without any validation.
    ///
    /// # Panics
    ///
    /// This function will panic if `LEN` is greater than 255.
    ///
    /// # Safety
    ///
    /// The caller guarantees that `id_bytes` contains 0 to `LEN` (inclusive) ASCII bytes.
    const unsafe fn from_bytes_unchecked(id_bytes: &[u8]) -> Self {
        assert_valid_len::<LEN>();

        // SAFETY: `LEN` is at most 255.
        let len = unsafe { NonMaxU8::new_unchecked(id_bytes.len() as _) };
        let bytes = {
            let mut buf = [0; LEN];
            // TODO: Remove this when const slice indexing is stable
            // SAFETY: The caller guarantees that `id_bytes` is at most LEN bytes.
            let slice =
                unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr(), id_bytes.len()) };
            slice.copy_from_slice(id_bytes);
            buf
        };
        Self { len, bytes }
    }

    /// Pushes a string on to the end of the identifier.
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let mut id = AsciiIdentifier::from_str("abc").unwrap();
    /// id.push_str("123").unwrap();
    /// assert_eq!(id, "abc123");
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - `s` is not ASCII
    /// - `s` is too long to fit in the remaining buffer space
    pub const fn push_str(&mut self, s: &str) -> Result<(), AsciiIdentifierError> {
        self.push_bytes(s.as_bytes())
    }

    /// Pushes ASCII bytes on to the end of the identifier.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - `s` is not ASCII
    /// - `s` is too long to fit in the remaining buffer space
    pub const fn push_bytes(&mut self, s: &[u8]) -> Result<(), AsciiIdentifierError> {
        let res = validate_ascii_slice(LEN - self.len(), s);
        if res.is_ok() {
            // SAFETY: `bytes` is ASCII and will fit in the buffer
            unsafe { self.push_bytes_unchecked(s) };
        }
        res
    }

    /// Pushes bytes on to the end of the identifier without any validation.
    ///
    /// # Safety
    ///
    /// The caller guarantees that:
    /// - `bytes` contains valid ASCII
    /// - `self.len() + bytes.len() <= LEN`
    const unsafe fn push_bytes_unchecked(&mut self, bytes: &[u8]) {
        // 1. Copy bytes into the buffer

        // TODO: Update to something like this when const slice indexing is stable:
        // self.bytes[old_len..new_len].copy_from_slice(bytes)
        // SAFETY: `self.len()` is less than or equal to the buffer length
        let ptr = unsafe { self.bytes.as_mut_ptr().add(self.len()) };
        // SAFETY: The caller guarantees that `self.len() + bytes.len() <= LEN`.
        unsafe { core::slice::from_raw_parts_mut(ptr, bytes.len()) }.copy_from_slice(bytes);

        // 2. Update the length

        // SAFETY: The caller guarantees that `self.len() + bytes.len() <= LEN`.
        self.len = unsafe { NonMaxU8::new_unchecked((self.len() + bytes.len()) as _) };
    }

    /// Returns the length of the identifier.
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let id = AsciiIdentifier::from_str("hello!").unwrap();
    /// assert_eq!(id.len(), 6);
    /// ```
    #[allow(
        clippy::len_without_is_empty,
        reason = "`AsciiIdentifier` cannot be empty"
    )]
    pub const fn len(&self) -> usize {
        self.len.get() as _
    }

    /// Returns the identifier's ASCII bytes.
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let id = AsciiIdentifier::from_str("hello!").unwrap();
    /// assert_eq!(id.as_bytes(), &[0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x21]);
    /// ```
    pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: `bytes` contains at least `len` bytes
        unsafe { core::slice::from_raw_parts(self.bytes.as_ptr(), self.len()) }
    }

    /// Returns the identifier's ASCII bytes as a [`str`].
    ///
    /// # Example
    ///
    /// ```
    /// # use ascii_identifier::AsciiIdentifier16 as AsciiIdentifier;
    /// let id = AsciiIdentifier::from_str("hello!").unwrap();
    /// assert_eq!(id.as_str(), "hello!");
    /// ```
    pub const fn as_str(&self) -> &str {
        // SAFETY: Bytes are guaranteed to be ASCII, which is a subset of UTF-8
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl<const LEN: usize> Default for AsciiIdentifier<LEN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const LEN: usize> Hash for AsciiIdentifier<LEN> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<const LEN: usize> PartialEq<&'_ str> for AsciiIdentifier<LEN> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const LEN: usize> PartialEq<AsciiIdentifier<LEN>> for &'_ str {
    fn eq(&self, other: &AsciiIdentifier<LEN>) -> bool {
        *self == other.as_str()
    }
}

impl<const LEN: usize> AsRef<str> for AsciiIdentifier<LEN> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const LEN: usize> AsRef<[u8]> for AsciiIdentifier<LEN> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const LEN: usize> Display for AsciiIdentifier<LEN> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const LEN: usize> FromStr for AsciiIdentifier<LEN> {
    type Err = AsciiIdentifierError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

impl<const LEN: usize> fmt::Write for AsciiIdentifier<LEN> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s).map_err(|_| fmt::Error)
    }
}

/// Errors returned when attempting to construct an [`AsciiIdentifier`].
#[derive(Error, Debug)]
pub enum AsciiIdentifierError {
    /// The string or byte slice was not ASCII.
    #[error("must contain only ASCII characters")]
    NotAscii,
    /// The string or byte slice was too long to fit in the identifier.
    #[error("contained too many characters")]
    TooLong,
}

#[inline]
const fn assert_valid_len<const LEN: usize>() {
    if const { LEN > 255 } {
        panic!("`AsciiIdentifier<LEN>` may have a `LEN` of at most 255");
    }
}

#[inline]
const fn validate_ascii_slice(max_len: usize, s: &[u8]) -> Result<(), AsciiIdentifierError> {
    if s.len() > max_len {
        return Err(AsciiIdentifierError::TooLong);
    }

    if !s.is_ascii() {
        return Err(AsciiIdentifierError::NotAscii);
    }

    Ok(())
}

/// A [`format!`]-like macro that constructs [`AsciiIdentifier`]s.
///
/// ```
/// # use ascii_identifier::{AsciiIdentifier16, ascii_ident};
/// let id: AsciiIdentifier16 = ascii_ident!("{}{}", "abc", 123);
/// assert_eq!(id, "abc123");
/// ```
#[macro_export]
macro_rules! ascii_ident {
    ($fmt:expr) => {{
        use ::std::fmt::Write;
        let mut id = $crate::AsciiIdentifier::new();
        write!(id, $fmt).unwrap();
        id
    }};
    ($fmt:expr, $($args:tt)*) => {{
        use ::std::fmt::Write;
        let mut id = $crate::AsciiIdentifier::new();
        write!(id, $fmt, $($args)*).unwrap();
        id
    }};
}

#[cfg(feature = "serde")]
mod serde_impl {
    use serde::{Deserialize, Serialize};

    use super::*;

    impl<const LEN: usize> Serialize for AsciiIdentifier<LEN> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de, const LEN: usize> Deserialize<'de> for AsciiIdentifier<LEN> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            <&str>::deserialize(deserializer)?
                .parse()
                .map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_equality() {
        assert_eq!(AsciiIdentifier16::from_str("abcd").unwrap(), "abcd");
        assert_eq!("abcd", AsciiIdentifier16::from_str("abcd").unwrap());
        assert_ne!(AsciiIdentifier16::from_str("abcd").unwrap(), "efgh");
        assert_ne!("efgh", AsciiIdentifier16::from_str("abcd").unwrap());
    }

    #[test]
    fn parse_display_roundtrip() {
        assert_eq!(
            AsciiIdentifier16::from_str("hello-ident")
                .unwrap()
                .to_string(),
            "hello-ident"
        );
    }

    #[test]
    fn niche_optimized() {
        assert_eq!(
            core::mem::size_of::<AsciiIdentifier16>(),
            core::mem::size_of::<Option<AsciiIdentifier16>>()
        );
    }

    #[test]
    fn empty_identifier() {
        assert_eq!(AsciiIdentifier16::new(), "");
        assert_eq!(AsciiIdentifier16::default(), "");
        assert_eq!(AsciiIdentifier16::from_bytes(&[]).unwrap(), "");
        assert_eq!(AsciiIdentifier16::from_str("").unwrap(), "");
    }

    #[test]
    fn push_overflow_fails() {
        let mut id = AsciiIdentifier16::from_bytes(&[0; 15]).unwrap();
        assert!(matches!(
            id.push_bytes(&[0]).unwrap_err(),
            AsciiIdentifierError::TooLong,
        ));

        let mut id = AsciiIdentifier16::from_str("0123456789abcde").unwrap();
        assert!(matches!(
            id.push_str("f").unwrap_err(),
            AsciiIdentifierError::TooLong,
        ));
    }

    #[test]
    fn push_non_ascii_fails() {
        let mut id = AsciiIdentifier16::new();
        assert!(matches!(
            id.push_bytes(&[0xFF]).unwrap_err(),
            AsciiIdentifierError::NotAscii,
        ));

        let mut id = AsciiIdentifier16::new();
        assert!(matches!(
            id.push_str("♥").unwrap_err(),
            AsciiIdentifierError::NotAscii,
        ));
    }

    #[test]
    fn too_long_identifier_fails() {
        assert!(matches!(
            AsciiIdentifier16::from_bytes(&[b'a'; 16]).unwrap_err(),
            AsciiIdentifierError::TooLong,
        ));

        // Wrap `u8` length
        assert!(matches!(
            AsciiIdentifier16::from_bytes(&[b'a'; 260]).unwrap_err(),
            AsciiIdentifierError::TooLong,
        ));

        assert!(matches!(
            AsciiIdentifier16::from_str("0123456789abcdef").unwrap_err(),
            AsciiIdentifierError::TooLong,
        ));
    }

    #[test]
    fn non_ascii_identifier_fails() {
        assert!(matches!(
            AsciiIdentifier16::from_bytes(&[0xFF]).unwrap_err(),
            AsciiIdentifierError::NotAscii,
        ));

        assert!(matches!(
            AsciiIdentifier16::from_str("♥").unwrap_err(),
            AsciiIdentifierError::NotAscii,
        ));
    }

    #[test]
    #[should_panic(expected = "`AsciiIdentifier<LEN>` may have a `LEN` of at most 255")]
    fn length_greater_than_255_from_bytes_panics() {
        AsciiIdentifier::<256>::from_bytes(&[b'a'; 256]).unwrap();
    }

    #[test]
    #[should_panic(expected = "`AsciiIdentifier<LEN>` may have a `LEN` of at most 255")]
    fn length_greater_than_255_new_panics() {
        AsciiIdentifier::<256>::new();
    }
}
