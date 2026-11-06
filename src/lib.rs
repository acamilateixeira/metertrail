//! MeterTrail core: the event ledger and the batch projection.
//!
//! These modules use only the standard library, so the failure and replay
//! rules can be tested without the HTTP layer or any external service.

pub mod ledger;
pub mod projection;
