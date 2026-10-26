//! PHP source indexing: file discovery, parsing and symbol lookup.

mod cache;
mod composer;
pub mod discover;
pub mod filter;
pub mod index;
pub mod line_index;
mod members;
pub mod phpdoc;
pub mod summary;

pub use filter::PathFilter;
pub use index::{ClassRef, Index, IndexStats, Location, UpdateStats};
