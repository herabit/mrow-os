use std::path::PathBuf;

use anyhow::{anyhow, Context};
use bytemuck::checked::try_from_bytes_mut;
use cargo_metadata::camino::Utf8PathBuf;
use mrow_common::mbr::MasterBootRecord;
use tokio::{
    fs::File,
    io::{AsyncWrite, AsyncWriteExt},
    join,
};

use crate::{
    cargo::CargoBuild,
    util::{apply_context, Env, ObjCopy},
};

/// Struct for building a bios bootloader.
pub struct BiosBuilder<'a> {
    pub env: &'a Env,
    pub profile: &'a str,
    pub code16_target: Utf8PathBuf,
    pub code16_pic_target: Utf8PathBuf,
}

impl<'a> BiosBuilder<'a> {
    pub fn new(env: &'a Env, profile: &'a str) -> Self {
        Self {
            env,
            profile,
            code16_target: env.metadata.workspace_root.join("i386-code16.json"),
            code16_pic_target: env.metadata.workspace_root.join("i386-code16-pic.json"),
        }
    }

    /// Builds the bios bootloader and stores it in a file.
    pub async fn build_and_save(
        &self,
        stdout: &mut File,
        stderr: &mut File,
        path: &mut PathBuf,
    ) -> Result<(Vec<u8>, File), Vec<anyhow::Error>> {
        let bootloader = self
            .build(stdout, stderr)
            .await
            .map_err(apply_context(|| "building bootloader"))?;

        self.env.build_dir.as_std_path().clone_into(path);
        path.push("bios-boot.bin");

        let mut file = File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path.as_path())
            .await
            .context("creating bootloader file")
            .map_err(|err| vec![err])?;

        file.write_all(bootloader.as_slice())
            .await
            .context("writing bootloader file")
            .map_err(|err| vec![err])?;

        file.flush()
            .await
            .context("flushing bootloader file")
            .map_err(|err| vec![err])?;

        Ok((bootloader, file))
    }

    /// Builds the bios bootloader.
    pub async fn build(
        &self,
        stdout: &mut File,
        stderr: &mut File,
    ) -> Result<Vec<u8>, Vec<anyhow::Error>> {
        let stage_1 = {
            let mut stdout = stdout.try_clone().await.map_err(|err| vec![err.into()])?;
            let mut stderr = stderr.try_clone().await.map_err(|err| vec![err.into()])?;

            async move {
                self.build_stage1(&mut stdout, &mut stderr)
                    .await
                    .map_err(apply_context(|| "building stage 1"))
            }
        };

        let stage_2 = async {
            self.build_stage2(stdout, stderr)
                .await
                .map_err(apply_context(|| "building stage 2"))
            // .context("building stage 2")
        };

        let (mut stage_1, stage_2) = match join!(stage_1, stage_2) {
            (Ok(stage_1), Ok(stage_2)) => (stage_1, stage_2),
            (Ok(_), Err(err_2)) => return Err(err_2),
            (Err(err_1), Ok(_)) => return Err(err_1),
            (Err(mut err_1), Err(mut err_2)) => {
                err_1.append(&mut err_2);

                return Err(err_1);
            }
        };

        // let (mut stage_1, stage_2) = try_join!(stage_1, stage_2).context("building stages")?;

        let mbr = try_from_bytes_mut::<MasterBootRecord>(&mut stage_1)
            .context("getting master boot record")
            .map_err(|err| vec![err])?;

        if stage_2.is_empty() {
            return Err(vec![anyhow!("stage 2 loader must not be empty")]);
        } else if stage_2.len() % 512 != 0 {
            return Err(vec![anyhow!(
                "stage 2 loader size must be a multiple of 512"
            )]);
        }

        let stage_2_sectors = u32::try_from(stage_2.len() / 512)
            .context("stage 2 loader sector size must fit in a u32")
            .map_err(|err| vec![err])?;

        let bootloader_parition = &mut mbr.partition_table.entries[0];

        bootloader_parition.flags |= 0x80;
        bootloader_parition.set_start_lba(1);
        bootloader_parition.set_sector_len(stage_2_sectors);

        let mut bootloader = stage_1;
        bootloader.extend_from_slice(&stage_2);

        Ok(bootloader)
    }

    /// Builds the stage 2 loader.
    pub async fn build_stage2<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> Result<Vec<u8>, Vec<anyhow::Error>>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        let package = self
            .env
            .metadata
            .packages
            .iter()
            .find(|p| p.name == "mrow-bios-stage-2")
            .context("failed to find stage-2")
            .map_err(|err| vec![err])?;

        // Build it
        CargoBuild {
            package: &package.name,
            target: self.code16_pic_target.as_str(),
            profile: self.profile,
            build_std: Some(&["core", "compiler_builtins"]),
            build_std_features: &["compiler-builtins-mem"],
            ..self.env.cargo_build()
        }
        .run(&mut tokio::io::empty(), stdout, stderr)
        .await?;

        let input = self.env.target_path(
            self.code16_pic_target.file_stem(),
            Some(self.profile),
            Some(&package.name),
        );
        let output = self.env.build_path(&package.name, Some("bin"));

        let stage_2 = ObjCopy {
            input: input.as_str(),
            output: output.as_str(),
            output_format: Some("binary"),
            ..self.env.objcopy()
        }
        .run(stdout, stderr)
        .await?;

        Ok(stage_2)
    }

    /// Builds the stage 1 bios bootloader without modifying the master boot record.
    pub async fn build_stage1<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> Result<Vec<u8>, Vec<anyhow::Error>>
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
            .context("failed to find stage-1")
            .map_err(|err| vec![err])?;

        // Build it
        CargoBuild {
            package: &package.name,
            target: self.code16_target.as_str(),
            profile: self.profile,
            build_std: Some(&["core", "compiler_builtins"]),
            build_std_features: &["compiler-builtins-mem"],
            ..self.env.cargo_build()
        }
        .run(&mut tokio::io::empty(), stdout, stderr)
        .await?;

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
        .await
        .map_err(apply_context(|| "running objcopy on bios-stage-1"))?;

        Ok(stage_1)
    }
}
