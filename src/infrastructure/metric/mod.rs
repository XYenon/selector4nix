//! In-process metric store.
//!
//! Write path: lifecycle methods on `XxxMetric` (`begin` / `record_bytes` / drop).
//! Read path: status and future export adapters query the same store.

mod nar_transfer;
mod store;

pub use nar_transfer::{NarTransferAttrs, NarTransferHandle, NarTransferMetric, NarTransferSample};
pub use store::MetricStore;
