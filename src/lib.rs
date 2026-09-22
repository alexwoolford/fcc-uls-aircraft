//! Weekly FCC ULS Part 87 Active aircraft radio licenses.
//! A label, not a lead: the crate does not emit a name-match score.

pub mod db;
pub mod download;
pub mod ingest;
pub mod parse;
pub mod time;

pub use db::{open, open_work, WorkDb, DB_NAME};
pub use ingest::{ingest, IngestOptions, IngestStats, DEFAULT_MIN_HD_ROWS};
pub use parse::canonical_n_number;
pub use time::utc_iso;
