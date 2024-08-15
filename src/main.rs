use std::process::ExitCode;

use anyhow::Context;
use bios::BiosBuilder;

use tokio::{
    fs::File,
    io::{self, AsyncWriteExt},
    runtime,
};
use util::apply_context;

pub mod bios;
pub mod cargo;
pub mod util;

fn main() -> ExitCode {
    let mut errors = Vec::new();

    let runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting tokio runtime");

    match runtime {
        Ok(runtime) => runtime.block_on(run(&mut errors)),
        Err(err) => errors.push(err),
    }

    if errors.is_empty() {
        return ExitCode::SUCCESS;
    }

    for (index, error) in errors.iter().enumerate() {
        eprintln!("Error[{index}]: {error:?}");
    }

    ExitCode::FAILURE
}

async fn run(errors: &mut Vec<anyhow::Error>) {
    let (mut stdout, mut stderr) = (None, None);

    let result = run_inner(&mut stdout, &mut stderr).await;

    errors.extend(result.err().into_iter().flatten());

    if let Some(stdout) = &mut stdout {
        let result = stdout.sync_all().await.context("syncing stdout.log");
        errors.extend(result.err());
    }

    if let Some(stderr) = &mut stderr {
        let result = stderr.sync_all().await.context("syncing stderr.log");
        errors.extend(result.err());
    }
}

async fn run_inner(
    stdout: &mut Option<File>,
    stderr: &mut Option<File>,
) -> Result<(), Vec<anyhow::Error>> {
    let mut scratch = Vec::new();
    let env = util::load_env(&mut io::empty(), &mut scratch).await?;

    let (_stdout, _stderr) = env
        .setup_build(true)
        .await
        .context("setting up build environment")
        .map_err(|err| vec![err])?;

    let (stdout, stderr) = (stdout.insert(_stdout), stderr.insert(_stderr));

    stderr
        .write_all(&scratch)
        .await
        .context("writing environment loading output")
        .map_err(|err| vec![err])?;

    let bios_builder = BiosBuilder::new(&env, "bios-release");
    let mut bootloader_path = Default::default();

    bios_builder
        .build_and_save(stdout, stderr, &mut bootloader_path)
        .await
        .map_err(apply_context(|| "building and saving bios bootloader"))?;

    println!(
        "Built bootloader and saved to: {}",
        bootloader_path.display()
    );

    Ok(())
}
