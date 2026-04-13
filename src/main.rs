mod agent;
mod ai;
mod ai_controller;
mod app;
mod blocks;
mod commands;
mod config;
mod git;
mod license;
mod menu;
mod prompt;
mod renderer;
pub mod resources;
mod sandbox;
mod session;
mod system_info;
mod terminal;
mod ui;
mod usage;

#[tokio::main]
async fn main() {
    app::run();
}
