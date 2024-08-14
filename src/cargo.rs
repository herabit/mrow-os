use anyhow::ensure;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::Command,
};

use crate::util::run_command;

/// This exists just so that I can keep track of what the build-std crates are.
pub const BUILD_STD_CRATES: &[&str] = &[
    "std",
    "core",
    "alloc",
    "proc_macro",
    "panic_unwind",
    "compiler_builtins",
];

/// Type for building a cargo build command.
#[derive(Debug, Clone, Copy)]
pub struct CargoBuild<'a> {
    /// Path to cargo.
    pub command: &'a str,

    /// Package to build.
    pub package: &'a str,
    /// What target we're building for.
    pub target: &'a str,
    /// The profile we're building with.
    pub profile: &'a str,

    /// What features are enabled.
    ///
    /// Skips any keys that are empty.
    pub features: &'a [&'a str],
    /// Whether to disable default features.
    pub default_features: bool,

    /// List of additional arguments to pass to cargo.
    pub additional_args: &'a [&'a str],
    /// List of key value pairs representing environment variables to pass to cargo.
    ///
    /// Skips any keys that are empty.
    pub envs: &'a [(&'a str, &'a str)],

    /// List of build-std crates to use.
    pub build_std: Option<&'a [&'a str]>,
    /// List of features for build-std.
    ///
    /// Only used if `build_std` is set and this is not empy.
    pub build_std_features: &'a [&'a str],
}

impl<'a> CargoBuild<'a> {
    /// Creates a default cargo build command for a given environment.
    pub fn new(cargo_path: &'a str) -> Self {
        Self {
            command: cargo_path,
            package: "",
            target: "",
            profile: "",
            features: &[],
            default_features: true,
            additional_args: &[],
            envs: &[],
            build_std: None,
            build_std_features: &[],
        }
    }

    /// Creates a cargo build command and then executes it.
    pub async fn run<Stdin, Stdout, Stderr>(
        &self,
        stdin: &mut Stdin,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> anyhow::Result<()>
    where
        Stdin: AsyncRead + ?Sized + Unpin,
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        let status = run_command(&mut self.command(), stdin, stdout, stderr).await?;

        ensure!(status.success(), "cargo build exit status: {status}");

        Ok(())
    }

    /// Creates a cargo build command.
    pub fn command(&self) -> Command {
        let mut command = Command::new(self.command);

        let mut scratch = String::new();

        // Setup build-std
        if let Some(build_std) = self.build_std {
            scratch.push_str("-Zbuild-std");

            let mut ch = '=';

            for krate in build_std {
                scratch.push(ch);
                scratch.push_str(krate);

                ch = ',';
            }

            command.arg(scratch.as_str());
            scratch.clear();

            if !self.build_std_features.is_empty() {
                scratch.push_str("-Zbuild-std-features");

                ch = '=';

                for feature in self.build_std_features {
                    scratch.push(ch);
                    scratch.push_str(feature);

                    ch = ',';
                }

                command.arg(scratch.as_str());
                scratch.clear();
            }
        }

        // Start setting up the build.
        command.args(["build", "--package", self.package]);

        if !self.target.is_empty() {
            command.args(["--target", self.target]);
        }

        if !self.profile.is_empty() {
            command.args(["--profile", self.profile]);
        }

        if !self.additional_args.is_empty() {
            command.args(self.additional_args);
        }

        if !self.envs.is_empty() {
            command.envs(self.envs.iter().copied());
        }

        command.kill_on_drop(true);

        command
    }
}
