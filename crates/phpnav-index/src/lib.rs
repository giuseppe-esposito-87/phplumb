//! PHP source indexing: file discovery, parsing and symbol lookup.

pub mod discover;
pub mod index;
pub mod line_index;
pub mod summary;

pub use index::{Index, IndexStats, Location};
