use std::process::ExitStatus;

// use std::process::Command;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::Command,
};

use crate::util::run_command;

pub const BUILD_STD_CRATES: &[&str] = &[
    "std",
    "core",
    "alloc",
    "proc_macro",
    "panic_unwind",
    "compiler_builtins",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct CargoBuild<'a> {
    pub package: &'a str,

    pub target: &'a str,
    pub profile: &'a str,
    pub features: &'a [&'a str],

    pub additional_args: &'a [&'a str],
    pub envs: &'a [(&'a str, &'a str)],

    pub build_std_crates: Option<&'a [&'a str]>,
    pub build_std_features: &'a [&'a str],
}

impl<'a> CargoBuild<'a> {
    pub async fn run<Stdin, Stdout, Stderr>(
        &self,
        stdin: &mut Stdin,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> tokio::io::Result<ExitStatus>
    where
        Stdin: AsyncRead + ?Sized + Unpin,
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        run_command(&mut self.command(), stdin, stdout, stderr).await
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new("cargo");
        command.arg("+nightly");

        let mut scratch = String::new();

        // Setup build-std
        if let Some(crates) = self.build_std_crates {
            scratch.push_str("-Zbuild-std");

            let mut ch = '=';

            for krate in crates {
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

        command
    }
}
