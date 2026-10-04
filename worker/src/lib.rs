//! circles.mukoko.com: the public discovery pages and the ActivityPub
//! identity for Mukoko Circles.
//!
//! Every module but `entry` is plain Rust with no Workers dependency, tested
//! natively with `cargo test`. `entry` is the thin wasm fetch handler that
//! reads the request, calls the Nyuchi API and the static assets, and hands
//! the results to these functions.

pub mod ap;
pub mod api;
pub mod config;
pub mod model;
pub mod og;
pub mod pages;
pub mod route;
pub mod sitemap;
pub mod template;

#[cfg(target_arch = "wasm32")]
mod entry;
