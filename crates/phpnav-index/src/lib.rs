//! PHP source indexing: file discovery, parsing and symbol lookup.

mod composer;
pub mod discover;
pub mod filter;
pub mod index;
pub mod line_index;
pub mod summary;

pub use filter::PathFilter;
pub use index::{Index, IndexStats, Location, UpdateStats};
