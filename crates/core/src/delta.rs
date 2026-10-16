//! The `.zdelta` descriptor schema Zoi publishes and consumes.
//!
//! A `.zdelta` file is a container that [`zbsdiff`] defines: a zstd frame
//! holding a tar with a `meta.json` descriptor plus patch or payload entries.
//! `zbsdiff` carries `meta.json` without interpreting it, so the descriptor
//! schema belongs to whoever writes the container, and several projects reuse
//! the container for unrelated purposes.
//!
//! Zoi therefore names its own schema rather than borrowing the container's
//! identifier. Claiming `zdelta.v1` would make a Zoi package delta
//! indistinguishable from a `zync` sidecar, which uses the same container and
//! declares a block-oriented `regions` schema instead. A reader checking only
//! the container id would then accept a descriptor whose shape it cannot
//! interpret.
//!
//! Registry snapshots are not described here. They carry no `.zdelta` at all:
//! they are transferred with `zync`, against a `.zsync` control file.
//!
//! [`zbsdiff`]: https://crates.io/crates/zbsdiff

/// The format identifier stored in a `.zdelta` container's `meta.json`.
pub const ZOIDELTA_FORMAT_ID: &str = "zoidelta.v1";

/// The `type` discriminator of a `.zpa` package delta.
///
/// A package delta carries an `ops` array describing pool objects to patch or
/// ship whole.
pub const ZPA_DELTA_TYPE: &str = "zpa";
