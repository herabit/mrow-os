use core::str;
use std::{
    env::{self, consts::EXE_EXTENSION},
    fmt::Display,
    io::ErrorKind,
    process::{ExitStatus, Stdio},
};

use pin_project::pin_project;
use replace_with::replace_with_or_abort;
use tokio::{
    fs::{self, File},
    io::{self, empty, AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, AsyncWriteExt, Empty},
    join,
    process::Command,
    task::spawn_blocking,
    try_join,
};

use anyhow::{anyhow, Context};
use cargo_metadata::{camino::Utf8PathBuf, Metadata, MetadataCommand};

use crate::cargo::CargoBuild;

/// Helper type for stuff that may have a stream or may not.
#[pin_project(project = ProjMaybeStream)]
#[derive(Debug)]
pub enum MaybeStream<S> {
    Stream(#[pin] S),
    Empty(#[pin] Empty),
}

impl<S: AsyncRead> AsyncRead for MaybeStream<S> {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_read(cx, buf),
            ProjMaybeStream::Empty(s) => s.poll_read(cx, buf),
        }
    }
}

impl<S: AsyncBufRead> AsyncBufRead for MaybeStream<S> {
    #[inline]
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_fill_buf(cx),
            ProjMaybeStream::Empty(s) => s.poll_fill_buf(cx),
        }
    }

    #[inline]
    fn consume(self: std::pin::Pin<&mut Self>, amt: usize) {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.consume(amt),
            ProjMaybeStream::Empty(s) => s.consume(amt),
        }
    }
}

impl<S: AsyncSeek> AsyncSeek for MaybeStream<S> {
    #[inline]
    fn start_seek(
        self: std::pin::Pin<&mut Self>,
        position: std::io::SeekFrom,
    ) -> std::io::Result<()> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.start_seek(position),
            ProjMaybeStream::Empty(s) => s.start_seek(position),
        }
    }

    #[inline]
    fn poll_complete(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<u64>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_complete(cx),
            ProjMaybeStream::Empty(s) => s.poll_complete(cx),
        }
    }
}

impl<S: AsyncWrite> AsyncWrite for MaybeStream<S> {
    #[inline]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_write(cx, buf),
            ProjMaybeStream::Empty(s) => s.poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_flush(cx),
            ProjMaybeStream::Empty(s) => s.poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_shutdown(cx),
            ProjMaybeStream::Empty(s) => s.poll_shutdown(cx),
        }
    }

    #[inline]
    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.project() {
            ProjMaybeStream::Stream(s) => s.poll_write_vectored(cx, bufs),
            ProjMaybeStream::Empty(s) => s.poll_write_vectored(cx, bufs),
        }
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        match self {
            MaybeStream::Stream(s) => s.is_write_vectored(),
            MaybeStream::Empty(s) => s.is_write_vectored(),
        }
    }
}

impl<S> Default for MaybeStream<S> {
    fn default() -> Self {
        None.into()
    }
}

impl<S> From<Option<S>> for MaybeStream<S> {
    fn from(value: Option<S>) -> Self {
        match value {
            Some(stream) => Self::Stream(stream),
            None => Self::Empty(empty()),
        }
    }
}

impl<S> From<MaybeStream<S>> for Option<S> {
    #[inline]
    fn from(value: MaybeStream<S>) -> Self {
        match value {
            MaybeStream::Stream(stream) => Some(stream),
            MaybeStream::Empty(_) => None,
        }
    }
}

#[inline]
pub async fn io_copy<'a, R, W>(
    reader: Option<&'a mut R>,
    writer: Option<&'a mut W>,
) -> Result<u64, Vec<anyhow::Error>>
where
    R: AsyncRead + ?Sized + Unpin,
    W: AsyncWrite + ?Sized + Unpin,
{
    let mut reader: MaybeStream<_> = reader.into();
    let mut writer: MaybeStream<_> = writer.into();

    let result = io::copy(&mut reader, &mut writer)
        .await
        .context("copying bytes to writer");

    let flush_result = writer.flush().await.context("flushing writer");

    let mut errors = Vec::new();

    let amount = match result {
        Ok(amount) => amount,
        Err(err) => {
            errors.push(err);
            0
        }
    };

    errors.extend(flush_result.err());

    if errors.is_empty() {
        Ok(amount)
    } else {
        Err(errors)
    }
}

#[inline]
pub async fn run_command<Stdin, Stdout, Stderr>(
    command: &mut Command,
    stdin: &mut Stdin,
    stdout: &mut Stdout,
    stderr: &mut Stderr,
) -> Result<ExitStatus, Vec<anyhow::Error>>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stdout: AsyncWrite + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    let mut errors = Vec::new();

    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting command");

    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            errors.push(err);

            return Err(errors);
        }
    };

    let mut stdin_pipe = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let stdin_fut = io_copy(Some(stdin), stdin_pipe.as_mut());
    let stdout_fut = io_copy(stdout_pipe.as_mut(), Some(stdout));
    let stderr_fut = io_copy(stderr_pipe.as_mut(), Some(stderr));
    let child_fut = async { child.wait().await.context("waiting for child") };

    let (stdin_res, stdout_res, stderr_res, child_res) =
        join!(stdin_fut, stdout_fut, stderr_fut, child_fut);

    drop((stdin_pipe, stdout_pipe, stderr_pipe));

    errors.extend(
        [stdin_res, stderr_res, stdout_res]
            .into_iter()
            .filter_map(Result::err)
            .flatten(),
    );

    match child_res {
        Ok(status) if errors.is_empty() => Ok(status),
        Ok(_) => Err(errors),
        Err(err) => {
            errors.push(err);

            Err(errors)
        }
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
    ) -> Result<(), Vec<anyhow::Error>>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        // let (status, mut errors) =
        let status =
            run_command(&mut self.command(), &mut tokio::io::empty(), stdout, stderr).await?;

        if !status.success() {
            return Err(vec![anyhow!("llvm-objcopy returned with status {status}")]);
        }

        Ok(())
    }

    pub async fn run<Stdout, Stderr>(
        &self,
        stdout: &mut Stdout,
        stderr: &mut Stderr,
    ) -> Result<Vec<u8>, Vec<anyhow::Error>>
    where
        Stdout: AsyncWrite + ?Sized + Unpin,
        Stderr: AsyncWrite + ?Sized + Unpin,
    {
        self.exec(stdout, stderr).await?;

        tokio::fs::read(self.output)
            .await
            .with_context(|| format!("reading {:?}", self.output))
            .map_err(|err| vec![err])
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
    /// Cargo path
    pub cargo: Utf8PathBuf,
    /// Rustlib path
    pub rust_lib: Utf8PathBuf,
    /// Objcopy path
    pub objcopy: Utf8PathBuf,
}

pub async fn load_env<Stdin, Stderr>(
    stdin: &mut Stdin,
    stderr: &mut Stderr,
) -> Result<Env, Vec<anyhow::Error>>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    let host_target = host_target(stdin, stderr)
        .await
        .map_err(apply_context(|| "loading host target"))?;

    let sysroot = sysroot(stdin, stderr)
        .await
        .map_err(apply_context(|| "loading rust sysroot"))?;

    let cargo = {
        let mut path = sysroot.clone();
        path.extend(["bin", "cargo"]);
        path.set_extension(EXE_EXTENSION);

        path
    };

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

    let metadata = metadata(cargo.as_str(), stdin, stderr)
        .await
        .map_err(apply_context(|| "loading cargo metadata"))?;

    let build_dir = metadata.workspace_root.join("build");

    Ok(Env {
        metadata,
        build_dir,
        host_target,
        sysroot,
        rust_lib,
        objcopy,
        cargo,
    })
}

#[inline]
pub fn apply_context<C: Display + Send + Sync + Clone + 'static>(
    mut ctx: impl FnMut() -> C,
) -> impl FnMut(Vec<anyhow::Error>) -> Vec<anyhow::Error> {
    move |mut errors| {
        add_context(&mut errors, &mut ctx);

        errors
    }
}

#[inline]
pub fn add_context<C: Display + Send + Sync + Clone + 'static>(
    errors: &mut [anyhow::Error],
    mut ctx: impl FnMut() -> C,
) {
    errors
        .iter_mut()
        .for_each(|err| replace_with_or_abort(err, |err| err.context(ctx())))
}

#[inline]
async fn metadata<Stdin, Stderr>(
    cargo_path: &str,
    stdin: &mut Stdin,
    stderr: &mut Stderr,
) -> Result<Metadata, Vec<anyhow::Error>>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    let command = MetadataCommand::new()
        .no_deps()
        .cargo_path(cargo_path)
        .cargo_command();
    let mut command = Command::from(command);

    let mut output = Vec::new();
    let status = run_command(command.kill_on_drop(true), stdin, &mut output, stderr).await?;

    if !status.success() {
        return Err(vec![anyhow!("cargo exited with status: {status}")]);
    }

    String::from_utf8(output)
        .map_err(anyhow::Error::from)
        .and_then(|output| MetadataCommand::parse(output).map_err(Into::into))
        .context("parsing cargo metadata")
        .map_err(|err| vec![err])
}

#[inline]
async fn sysroot<Stdin, Stderr>(
    stdin: &mut Stdin,
    stderr: &mut Stderr,
) -> Result<Utf8PathBuf, Vec<anyhow::Error>>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    let mut output = Vec::new();

    let status = run_command(
        Command::new("rustc")
            .kill_on_drop(true)
            .args(["--print", "sysroot"]),
        stdin,
        &mut output,
        stderr,
    )
    .await?;

    if !status.success() {
        return Err(vec![anyhow!("rustc exited with status: {status}")]);
    }

    String::from_utf8(output)
        .map(|o| o.trim().into())
        .context("parsing sysroot")
        .map_err(|err| vec![err])
}

#[inline]
async fn host_target<Stdin, Stderr>(
    stdin: &mut Stdin,
    stderr: &mut Stderr,
) -> Result<String, Vec<anyhow::Error>>
where
    Stdin: AsyncRead + ?Sized + Unpin,
    Stderr: AsyncWrite + ?Sized + Unpin,
{
    let mut output = Vec::new();

    let status = run_command(
        Command::new("rustc")
            .kill_on_drop(true)
            .args(["--version", "--verbose"]),
        stdin,
        &mut output,
        stderr,
    )
    .await?;

    if !status.success() {
        return Err(vec![anyhow!("rustc exited with status: {status}")]);
    }
    String::from_utf8(output)
        .map_err(anyhow::Error::from)
        .and_then(|output| {
            output
                .lines()
                .find_map(|line| line.strip_prefix("host:"))
                .map(str::trim)
                .map(str::to_owned)
                .context("failed to find host")
        })
        .context("parsing rustc host")
        .map_err(|err| vec![err])
}

impl Env {
    /// Start building an objcopy command.
    pub fn objcopy(&self) -> ObjCopy<'_> {
        ObjCopy {
            command: self.objcopy.as_str(),
            ..Default::default()
        }
    }

    /// Start building a cargo build command.
    pub fn cargo_build(&self) -> CargoBuild<'_> {
        CargoBuild::new(self.cargo.as_str())
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

    pub async fn remove_build_dir(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.build_dir).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub async fn create_build_dir(&self) -> io::Result<()> {
        match fs::create_dir_all(&self.build_dir).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(()),
            Err(err) => Err(err),
        }
    }

    #[inline]
    async fn setup_log_file(&self, filename: &str) -> anyhow::Result<File> {
        let mut path = self.build_dir.clone();
        path.push(filename);
        path.set_extension("log");

        File::options()
            .append(true)
            .create(true)
            .open(path.as_path())
            .await
            .with_context(|| format!("opening log file at {path:?}"))
    }

    #[inline]
    async fn setup_dir(&self, reset: bool) -> anyhow::Result<(File, File)> {
        if reset {
            self.remove_build_dir()
                .await
                .context("removing previous build dir")?;
        }

        self.create_build_dir()
            .await
            .context("creating build dir")?;

        let setup_stdout = self.setup_log_file("stdout");
        let setup_stderr = self.setup_log_file("stderr");

        try_join!(setup_stdout, setup_stderr)
    }

    /// Setup the build environment.
    ///
    /// `reset_dir` indicates whether we should reset the build directory.
    ///
    /// Returns (stdout_file, stderr_file).
    pub async fn setup_build(&self, reset_dir: bool) -> anyhow::Result<(File, File)> {
        let cd = async {
            self.cd_workspace()
                .await
                .context("changing directory into workspace")
        };
        let build_dir = async {
            self.setup_dir(reset_dir)
                .await
                .context("setting up build directory")
        };

        try_join!(cd, build_dir).map(|(_, log_files)| log_files)
    }
}
