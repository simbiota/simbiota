//! Common traits and structs for implementing the detection system.
//!
//! The [`detector`] module contains the required tool to build
//! detectors for any given detection method, as well as a few useful
//! implementations.
//!
//! The [`hash`] module contains code related to hash based detection algorithms.

pub mod cache;
pub mod detector;
pub mod hash;
