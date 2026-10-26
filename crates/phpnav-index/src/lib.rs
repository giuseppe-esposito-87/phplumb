//! PHP source indexing: file discovery, parsing and symbol lookup.

mod cache;
mod composer;
pub mod discover;
pub mod filter;
mod hover;
pub mod index;
mod infer;
pub mod line_index;
mod members;
pub mod phpdoc;
pub mod summary;

pub use filter::PathFilter;
pub use hover::Hover;
pub use index::{ClassRef, Index, IndexStats, Location, UpdateStats};
