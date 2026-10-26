//! PHP source indexing: file discovery, parsing and symbol lookup.

mod cache;
mod calls;
mod composer;
pub mod discover;
pub mod filter;
mod hover;
pub mod index;
mod infer;
pub mod line_index;
mod members;
pub mod phpdoc;
mod references;
pub mod summary;
pub mod symbols;

pub use calls::{CallItem, CallKind, Calls};
pub use filter::PathFilter;
pub use hover::Hover;
pub use index::{ClassRef, Index, IndexStats, Location, UpdateStats, member_access_offsets};
pub use summary::RefKey;
