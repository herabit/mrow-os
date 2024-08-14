use anyhow::Context;
use bios::BiosBuilder;

use tokio::io;

pub mod bios;
pub mod cargo;
pub mod util;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let mut scratch = Vec::new();
    let env = util::load_env(&mut io::empty(), &mut scratch).await?;

    let (mut stdout, mut stderr) = env
        .setup_build(true)
        .await
        .context("setting up build environment")?;

    let bios_builder = BiosBuilder::new(&env, "release");
    let mut bootloader_path = Default::default();

    bios_builder
        .build_and_save(&mut stdout, &mut stderr, &mut bootloader_path)
        .await
        .context("building and saving bios bootloader")?;

    println!(
        "Built bootloader and saved to: {}",
        bootloader_path.display()
    );

    Ok(())
}
