//! ec-core — the heart of the Education Center Management System.
//!
//! Layers used across all phases:
//! - [`config`]: data directory, app settings, first-run helpers
//! - [`database`]: SQLite pool, pragmas, migrations, integrity checks
//! - [`common`]: shared primitives (Money, dates, sequences)
//!
//! Business/financial modules (students, courses, finance engine, ...) are
//! added here phase by phase. Nothing in this crate may depend on the Tauri
//! shell (`ec-app`) — the dependency direction is one-way.

pub mod common;
pub mod config;
pub mod database;
pub mod error;

pub use error::AppError;
pub type AppResult<T> = Result<T, AppError>;

/// Application identity constants.
pub const APP_NAME: &str = "Education Center Management System";
pub const BUNDLE_ID: &str = "com.educationcenter.desktop";
