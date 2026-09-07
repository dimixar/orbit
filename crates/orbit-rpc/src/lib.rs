//! `orbit-rpc` — a Rust client for the pi CLI RPC protocol.
//!
//! The pi CLI (`pi --mode rpc`) exposes a strict-JSONL protocol over stdio:
//! commands on stdin, events on stdout. This crate owns one pi child process
//! per [`PiClient`], routes responses, and streams typed [`Event`]s.
//!
//! Design rules (see AGENT.md):
//! - framing is LF-only; never split on Unicode separators
//! - envelopes parse loosely; unknown shapes flow through as raw JSON
//! - the client never blocks a UI frame: draining is `try_recv`-based

pub mod client;
pub mod types;

pub use client::{PiClient, PI_BIN_ENV};
pub use types::{AssistantMessageEvent, Command, CommandBody, Event};
