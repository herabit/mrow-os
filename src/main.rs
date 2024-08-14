use std::sync::Arc;

use anyhow::{ensure, Context};
use cargo::CargoBuild;
use cargo_metadata::camino::Utf8PathBuf;
#[allow(unused_imports)]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::{fs::File, io::BufWriter};
use util::{Env, ObjCopy};

pub mod cargo;
pub mod util;

fn main() -> anyhow::Result<()> {
    let env = util::load_env()?;

    // bios::build(environment, "release").context("building bios")?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(env))
}

async fn run(env: Arc<Env>) -> anyhow::Result<()> {
    let mut stdout = File::options()
        .append(true)
        .create(true)
        .open(env.build_dir.join("stdout.log"))
        .await?;

    let mut stdout = BufWriter::new(&mut stdout);

    let mut stderr = File::options()
        .append(true)
        .create(true)
        .open(env.build_dir.join("stderr.log"))
        .await?;

    let mut stderr = BufWriter::new(&mut stderr);

    let bios_builder = BiosBuilder::new(&env, "release");

    let stage_1 = bios_builder.build_stage1(&mut stdout, &mut stderr).await?;

    println!("Got {}", stage_1.len());

    Ok(())
}

pub struct BiosBuilder<'a> {
    pub env: &'a Env,
    pub profile: &'a str,
    pub code16_target: Utf8PathBuf,
}

impl<'a> BiosBuilder<'a> {
    pub fn new(env: &'a Env, profile: &'a str) -> Self {
        Self {
            env,
            profile,
            code16_target: env.metadata.workspace_root.join("i386-code16.json"),
        }
    }
    pub async fn build_stage1<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> anyhow::Result<Vec<u8>>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        let package = self
            .env
            .metadata
            .packages
            .iter()
            .find(|p| p.name == "mrow-bios-stage-1")
            .context("failed to find stage-1")?;

        let status = CargoBuild {
            package: &package.name,
            target: self.code16_target.as_str(),
            profile: self.profile,
            build_std_crates: Some(&["core", "compiler_builtins"]),
            build_std_features: &["compiler-builtins-mem"],
            ..Default::default()
        }
        .run(&mut tokio::io::empty(), stdout, stderr)
        .await?;

        ensure!(status.success(), "cargo build exit status: {status}");

        let input = self.env.target_path(
            self.code16_target.file_stem(),
            Some(self.profile),
            Some(&package.name),
        );
        let output = self.env.build_path(&package.name, Some("bin"));

        let stage_1 = ObjCopy {
            input: input.as_str(),
            output: output.as_str(),
            output_format: Some("binary"),
            ..self.env.objcopy()
        }
        .run(stdout, stderr)
        .await?;

        Ok(stage_1)
    }
}

// fn build_bios(build_dir: &Utf8Path, metadata: &Metadata, profile: &str) -> anyhow::Result<()> {
//     let target = metadata.workspace_root.join("i386-code16.json");

//     let stage_1 = metadata
//         .packages
//         .iter()
//         .find(|p| p.name == "mrow-bios-stage-1")
//         .context("failed to find stage-1")?;

//     let cargo_build = CargoBuild {
//         package: &stage_1.name,
//         target: target.as_str(),
//         profile,
//         features: &[],
//         additional_args: &[],
//         envs: &[],
//         build_std_crates: Some(&["core", "compiler_builtins"]),
//         build_std_features: &["compiler-builtins-mem"],
//     };

//     let status = cargo_build
//         .command()
//         .status()
//         .context("failed to start cargo")?;

//     if !status.success() {
//         return Err(anyhow!("cargo build failed: {status}"));
//     }

//     let path = Utf8PathBuf::from_iter(metadata.target_directory.iter().chain([
//         target.file_stem().unwrap(),
//         profile,
//         &stage_1.name,
//     ]));

//     let data = fs::read(&path).with_context(|| format!("reading {path:?}"))?;

//     Ok(())
// }
