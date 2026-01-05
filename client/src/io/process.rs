use futures_util::stream::{Stream, StreamExt};
use iced::futures;
use std::process::{ExitStatus, Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tokio_stream::wrappers::LinesStream;

/// Returns a stream of stdout/stderr lines of the Process
pub(crate) fn stream_process(
    cmd: &mut Command,
) -> Result<impl Stream<Item = ProcessUpdate> + use<>, tokio::io::Error> {
    // Avoid allocating a console
    #[cfg(windows)]
    cmd.creation_flags(windows_sys::Win32::System::Threading::DETACHED_PROCESS);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.kill_on_drop(false).spawn()?;

    let stdout = child.stdout.take().unwrap(); // Safe because we setup stdout & stderr beforehand
    let stderr = child.stderr.take().unwrap();

    let stdout = LinesStream::new(BufReader::new(stdout).lines());
    let stderr = LinesStream::new(BufReader::new(stderr).lines());
    // Merge stdout and stderr together
    //TODO: FIXME: this chains both futures, but the original code merged them
    // maybe use:? let reader = futures_util::future::select(stdout, stderr);
    let reader = stdout.chain(stderr);
    let exit_status = tokio::spawn(async move { child.wait().await });

    Ok(reader
        .map(|x| match x {
            Ok(x) => ProcessUpdate::Line(x),
            Err(e) => ProcessUpdate::Error(e.to_string()),
        })
        .chain(futures::stream::once(async {
            match exit_status.await {
                Ok(x) => match x {
                    Ok(x) => ProcessUpdate::Exit(x),
                    Err(e) => ProcessUpdate::Error(e.to_string()),
                },
                Err(e) => ProcessUpdate::Error(e.to_string()),
            }
        })))
}

#[derive(Clone, Debug)]
pub enum ProcessUpdate {
    Line(String),
    Exit(ExitStatus),
    Error(String),
}
