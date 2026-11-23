//! Shared, Bevy-free data layer for Omega.
//!
//! * [`ContentHash`] — deterministic change-detection fingerprint of serializable data.
//! * [`atomic`] — crash-safe file writing (temp file, fsync, rename).
//! * [`slot`] — versioned save slot with a manifest that is committed last.

pub mod atomic;
pub mod hash;
pub mod slot;

pub use hash::{ContentHash, hash_serializable};
pub use slot::{Manifest, SaveSlot, SlotError, CURRENT_SLOT_SCHEMA};
