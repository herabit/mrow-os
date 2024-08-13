use std::{
    fs::{self, File},
    io::ErrorKind,
};

use anyhow::{anyhow, Context};
use cargo::CargoBuild;
use cargo_metadata::{
    camino::{Utf8Path, Utf8PathBuf},
    Metadata, MetadataCommand,
};
use object::{read::elf::ElfFile32, Endianness};
use util::will_strip;

// pub mod bios;
pub mod cargo;
pub mod util;

fn main() -> anyhow::Result<()> {
    let metadata = MetadataCommand::new().no_deps().exec()?;
    let build_dir = metadata.workspace_root.join("build");

    match fs::create_dir_all(&build_dir) {
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
        err => err.with_context(|| format!("failed to create {build_dir:?}"))?,
    }

    build_bios(&build_dir, &metadata, "release").context("building bios bootloader")?;

    Ok(())
}

fn build_bios(build_dir: &Utf8Path, metadata: &Metadata, profile: &str) -> anyhow::Result<()> {
    let target = metadata.workspace_root.join("i386-code16.json");

    let stage_1 = metadata
        .packages
        .iter()
        .find(|p| p.name == "mrow-bios-stage-1")
        .context("failed to find stage-1")?;

    let cargo_build = CargoBuild {
        package: &stage_1.name,
        target: target.as_str(),
        profile,
        features: &[],
        additional_args: &[],
        envs: &[],
        build_std_crates: Some(&["core", "compiler_builtins"]),
        build_std_features: &["compiler-builtins-mem"],
    };

    let status = cargo_build
        .command()
        .status()
        .context("failed to start cargo")?;

    if !status.success() {
        return Err(anyhow!("cargo build failed: {status}"));
    }

    let path = Utf8PathBuf::from_iter(metadata.target_directory.iter().chain([
        target.file_stem().unwrap(),
        profile,
        &stage_1.name,
    ]));

    let data = fs::read(&path).with_context(|| format!("reading {path:?}"))?;

    let elf = ElfFile32::<Endianness, _>::parse(data.as_slice())
        .with_context(|| format!("parsing {path:?}"))?;

    let mut output = File::create(build_dir.join(&stage_1.name).with_extension("bin"))?;

    let bytes = util::objcopy_binary(
        &elf,
        &mut Vec::new(),
        &mut Vec::new(),
        0,
        &mut output,
        |section| !will_strip(section, true),
    )
    .with_context(|| format!("copying {path:?}"))?;

    println!("bios-stage-1 size: {}", bytes);

    Ok(())
}
