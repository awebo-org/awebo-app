//! Sandbox — microsandbox integration for isolated terminal environments.
//!
//! Architecture:
//! - `images`  — trusted image registry (OCI images for sandboxes)
//! - `config`  — per-sandbox configuration (resources, volumes, env)
//! - `manager` — sandbox lifecycle (create, stop, destroy) + tokio bridge
//! - `bridge`  — PTY bridge between microsandbox streams and alacritty terminal

pub mod bridge;
pub mod config;
pub mod images;
pub mod manager;
