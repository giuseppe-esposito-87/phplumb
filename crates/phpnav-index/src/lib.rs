//! PHP source indexing: file discovery, parsing and symbol lookup.

mod cache;
mod composer;
pub mod discover;
pub mod filter;
pub mod index;
pub mod line_index;
pub mod phpdoc;
pub mod summary;

pub use filter::PathFilter;
pub use index::{Index, IndexStats, Location, UpdateStats};
