//! # QUIC RTC Diagnostics
//!
//! Debugging and diagnostic tools for QUIC RTC.
//! Provides connection analysis, network profiling, and structured logging.

#![deny(missing_docs)]
#![warn(clippy::all)]

pub mod connection_analyzer;
pub mod network_profiler;
pub mod debug_logger;

// Re-export main types
pub use connection_analyzer::{ConnectionInfo, ConnectionStats};
pub use network_profiler::NetworkProfiler;