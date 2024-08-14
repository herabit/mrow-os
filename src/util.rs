use core::str;
use std::{
    env::{self, consts::EXE_EXTENSION},
    io::ErrorKind,
    process::{ExitStatus, Stdio},
    str::from_utf8,
    sync::Arc,
};

use tokio::{
    fs,
    io::{AsyncRead, AsyncWrite},
    process::Command,
    task::spawn_blocking,
    try_join,
};

use anyhow::{ensure, Context};
use cargo_metadata::{camino::Utf8PathBuf, Metadata, MetadataCommand};

#[inline]
pub async fn run_command<Stdin, Stdout, Stderr>(
    command: &mut Command,
    stdin: &mut Stdin,
    stdout: &mut Stdout,
    stderr: &mut Stderr,
) -> tokio::io::Result<ExitStatus>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stdout: AsyncWrite + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    #[inline]
    async fn io_copy<'a, R, W>(
        reader: Option<&'a mut R>,
        writer: Option<&'a mut W>,
    ) -> tokio::io::Result<u64>
    where
        R: AsyncRead + ?Sized + Unpin,
        W: AsyncWrite + ?Sized + Unpin,
    {
        use tokio::io;
        match (reader, writer) {
            (Some(reader), Some(writer)) => io::copy(reader, writer).await,
            (Some(reader), None) => io::copy(reader, &mut io::sink()).await,
            (None, Some(writer)) => io::copy(&mut io::empty(), writer).await,
            _ => Ok(0),
        }
    }

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin_pipe = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let stdin_fut = io_copy(Some(stdin), stdin_pipe.as_mut());
    let stdout_fut = io_copy(stdout_pipe.as_mut(), Some(stdout));
    let stderr_fut = io_copy(stderr_pipe.as_mut(), Some(stderr));

    match try_join!(child.wait(), stdin_fut, stdout_fut, stderr_fut) {
        Ok((status, _stdin_bytes, _stdout_bytes, _stderr_bytes)) => Ok(status),
        Err(err) => Err(err),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ObjCopy<'a> {
    /// Path to the objcopy command.
    pub command: &'a str,
    /// Path to the input file.
    pub input: &'a str,
    /// Input file format.
    pub input_format: Option<&'a str>,
    /// Path to the output file.
    pub output: &'a str,
    /// Output file format.
    pub output_format: Option<&'a str>,
}

impl<'a> ObjCopy<'a> {
    pub fn command(&self) -> Command {
        let mut command = Command::new(self.command);

        if let Some(format) = self.input_format {
            command.args(["-I", format]);
        }

        if let Some(format) = self.output_format {
            command.args(["-O", format]);
        }

        command.args([self.input, self.output]);

        command
    }

    pub async fn exec<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> anyhow::Result<()>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        let status =
            run_command(&mut self.command(), &mut tokio::io::empty(), stdout, stderr).await?;

        ensure!(
            status.success(),
            "llvm-objcopy returned with status {status}"
        );

        Ok(())
    }

    pub async fn run<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> anyhow::Result<Vec<u8>>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        self.exec(stdout, stderr).await?;

        tokio::fs::read(self.output)
            .await
            .with_context(|| format!("reading {:?}", self.output))
    }
}

#[derive(Debug)]
pub struct Env {
    // Workspace metadata
    pub metadata: Metadata,
    /// Build directory
    pub build_dir: Utf8PathBuf,
    /// Target host
    pub host_target: String,
    /// Sysroot path
    pub sysroot: Utf8PathBuf,
    /// Rustlib path
    pub rust_lib: Utf8PathBuf,
    /// Objcopy path
    pub objcopy: Utf8PathBuf,
}

impl Env {
    /// Start building an objcopy command.
    pub fn objcopy(&self) -> ObjCopy<'_> {
        ObjCopy {
            command: self.objcopy.as_str(),
            ..Default::default()
        }
    }

    /// Create a path in the target dir.
    pub fn target_path(
        &self,
        target: Option<&str>,
        profile: Option<&str>,
        file: Option<&str>,
    ) -> Utf8PathBuf {
        let mut dir = self.metadata.target_directory.clone();

        if let Some(target) = target.filter(|s| !s.is_empty()) {
            dir.push(target);
        }

        dir.push(profile.filter(|s| !s.is_empty()).unwrap_or("debug"));

        if let Some(file) = file.filter(|s| !s.is_empty()) {
            dir.push(file);
        }

        dir
    }

    /// Create a path in the build dir
    pub fn build_path(&self, package: &str, extension: Option<&str>) -> Utf8PathBuf {
        let mut path = self.build_dir.clone();

        path.push(package);
        path.set_extension(extension.unwrap_or(""));

        path
    }

    pub async fn cd_workspace(&self) -> anyhow::Result<()> {
        let dir = self.metadata.workspace_root.clone();
        spawn_blocking(move || env::set_current_dir(dir)).await??;
        Ok(())
    }

    pub async fn create_build_dir(&self) -> anyhow::Result<()> {
        match fs::create_dir_all(&self.build_dir).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

pub fn load_env() -> anyhow::Result<Arc<Env>> {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("getting cargo metadata")?;

    let build_dir = metadata.workspace_root.join("build");
    let host_target = host_target().context("finding host target")?;
    let sysroot = sysroot().context("finding sysroot")?;

    let rust_lib = {
        let mut path = sysroot.clone();

        path.extend(["lib", "rustlib", host_target.as_str()]);

        path
    };

    let objcopy = {
        let mut path = rust_lib.clone();

        path.extend(["bin", "llvm-objcopy"]);
        path.set_extension(EXE_EXTENSION);

        path
    };

    Ok(Arc::new(Env {
        metadata,
        build_dir,
        host_target,
        sysroot,
        rust_lib,
        objcopy,
    }))
}

#[inline]
fn sysroot() -> anyhow::Result<Utf8PathBuf> {
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()?;

    ensure!(
        output.status.success(),
        "rustc returned with status {}",
        output.status
    );

    let sysroot = from_utf8(&output.stdout).context("parsing stdout")?.trim();

    Ok(sysroot.into())
}

#[inline]
fn host_target() -> anyhow::Result<String> {
    let output = std::process::Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()?;

    ensure!(
        output.status.success(),
        "rustc returned with status {}",
        output.status
    );

    let output = str::from_utf8(&output.stdout).context("parsing stdout")?;

    let output = output
        .lines()
        .find_map(|line| line.strip_prefix("host:"))
        .map(str::trim)
        .context("finding host in rustc version")?;

    Ok(output.to_owned())
}
