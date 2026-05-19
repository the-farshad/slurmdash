//! Slurm command wrappers.
//!
//! Each submodule wraps a single Slurm CLI tool. Functions take an
//! [`crate::ssh::Runner`] and return typed results.

pub mod hostlist;
pub mod model;
pub mod parse;
pub mod reason;
pub mod scancel;
pub mod scontrol;
pub mod sinfo;
pub mod squeue;
pub mod state;
pub mod version;

pub use model::{Job, JobDetails};
pub use state::JobState;
