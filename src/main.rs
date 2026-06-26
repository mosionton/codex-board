use anyhow::Result;

pub mod app;
pub mod provider_config;
pub mod session_store;
pub mod ui;

fn main() -> Result<()> {
    app::run()
}
