use cargo_metadata::camino::Utf8PathBuf;

use crate::util::Env;

// pub fn build(environment: &Environment, profile: &str, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> anyhow::Result<Utf8PathBuf> {
//     let target = environment.metadata.workspace_root.join("i386-code16.json");

//     build_stage_1(environment, &target, profile, "0")
// }

// pub fn build_stage_1(
//     environment: &Environment,
//     target: &Utf8Path,
//     profile: &str,
//     stage_2_size: &str,
// ) -> anyhow::Result<Utf8PathBuf> {
//     let package = environment
//         .metadata
//         .packages
//         .iter()
//         .find(|p| p.name == "mrow-bios-stage-1")
//         .context("failed to find stage 1")?;

//     let output = CargoBuild {
//         package: &package.name,
//         target: target.as_str(),
//         profile,
//         envs: &[("MROW_STAGE_2_SIZE", stage_2_size)],
//         build_std_crates: Some(&["core", "compiler_builtins"]),
//         build_std_features: &["compiler-builtins-mem"],
//         ..Default::default()
//     }
//     .command()
//     .output()?;

//     stdout.extend_from_slice(&output.stdout);
//     stderr.extend_from_slice(&output.stderr);

//     ensure!(
//         output.status.success(),
//         "cargo build failed with status: {}",
//         output.status
//     );

//     let input = environment.target_path(target.file_stem(), Some(profile), Some(&package.name));
//     let output = environment.build_path(&package.name, Some("bin"));

//     ObjCopy {
//         input: input.as_str(),
//         output: output.as_str(),
//         output_format: Some("binary"),
//         ..environment.objcopy()
//     }
//     .exec()?;

//     Ok(output)
// }
