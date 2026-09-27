//! Rust-owned application domain. Qt and engine types do not cross this crate.

/// Stable identity for a future browser tab.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TabId(u64);

impl TabId {
    /// Creates an ID from an application-assigned numeric value.
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric value for serialization and FFI adapters.
    pub const fn as_raw(self) -> u64 {
        self.0
    }
}
