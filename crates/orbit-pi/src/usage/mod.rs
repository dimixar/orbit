//! The **Usage** page: where the agent's work is measured.
//!
//! Layered deliberately, so the rendering path never touches a record:
//!
//! ```text
//! pi session store (~/.pi/agent/sessions/**.jsonl)
//!         ↓  collect::UsageScanner      — incremental, off-thread
//! collect::UsageIndex                   — normalized records + interned tables
//!         ↓  aggregate::UsageSnapshot   — one pass per (index, filter)
//! page::UsagePage                       — cached snapshot + filter/view state
//!         ↓  view / chart / filters     — GPUI, aggregates only
//!         ↓  table                      — rows adapted to GPUI Kit's delegate
//! ```
//!
//! Accounting rules (what counts as a request, how cache hit rate is defined,
//! why retries are unavailable) are documented once, in [`model`].
//!
//! The data tables and the timeline plot are `gpui-component` widgets rather
//! than hand-rolled ones, themed through [`kit`] so they render in this app's
//! palette.

pub mod aggregate;
pub mod chart;
pub mod collect;
pub mod filters;
pub mod format;
pub mod kit;
pub mod model;
pub mod page;
pub mod table;
pub mod view;

#[cfg(test)]
mod tests;
