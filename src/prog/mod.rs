use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc, process::ExitStatus,
};

use duct::ReaderHandle;
use flume::{Receiver, Sender};
use log::trace;
use thiserror::Error;
use threadpool::ThreadPool;

pub struct Context<Msg, Handle, Error: std::error::Error, Output> {
    receiver: Receiver<ProgressMessage<Msg, Error, Output>>,
    handle: Handle,
}

pub enum ProgressMessage<Msg, Error, Output> {
    Update(Msg),
    Failed(Error),
    Done(Output),
}

impl<Msg, Handle, Error: std::error::Error, Output> Context<Msg, Handle, Error, Output> {
    pub fn run<F>(spawn_task: F) -> Self
    where
        F: FnOnce(Sender<ProgressMessage<Msg, Error, Output>>) -> Handle,
    {
        let (sender, receiver) = flume::unbounded::<ProgressMessage<Msg, Error, Output>>();
        let handle = spawn_task(sender);
        Self { receiver, handle }
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    pub fn rx(&self) -> &Receiver<ProgressMessage<Msg, Error, Output>> {
        &self.receiver
    }
}

#[derive(Error, Debug)]
pub enum CmdError {
    #[error("io error while running script")]
    Io(#[from] std::io::Error),

    #[error("failed to read command output")]
    InvalidOutput(#[from] std::string::FromUtf8Error),

    #[error("command did not complete after reader closed")]
    CommandNotDone,
}

pub type CmdContext<T> = Context<String, Arc<ReaderHandle>, CmdError, (ExitStatus, T)>;

pub fn run_script<P: AsRef<Path>, T: 'static + Send>(
    pool: &ThreadPool,
    parse_output: fn(String) -> T,
    script: P,
    env: &[(&str, &str)],
) -> Result<CmdContext<T>, CmdError> {
    if log::log_enabled!(log::Level::Trace) {
        let script_path = script.as_ref();
        let contents = fs::read_to_string(&script_path).unwrap_or_else(|_| "".to_owned());
        trace!("Running script `{script_path:?}` with content: \n{contents}");
    }

    let mut expr = duct::cmd!(script.as_ref())
        .stdout_capture()
        .stderr_capture()
        .unchecked();

    trace!("Setting env for script:\n{env:#?}");

    for (key, val) in env {
        expr = expr.env(key, val);
    }

    let reader = Arc::new(expr.stderr_reader()?);

    let context = CmdContext::<T>::run(|tx| {
        let ctx_reader = reader.clone();

        pool.execute(move || {
            let mut lines = BufReader::new(&*ctx_reader);
            let mut buffer = String::new();

            loop {
                buffer.clear();

                match lines.read_line(&mut buffer) {
                    // Reader has signaled that the command is done, so
                    // we try to read stdout and return the result
                    Ok(0) => {
                        // Guaranteed to return successfully
                        match ctx_reader.try_wait() {
                            Ok(Some(output)) => {
                                match String::from_utf8(output.stdout.clone()) {
                                    Ok(result) => {
                                        let _ = tx.send(ProgressMessage::Done((
                                            output.status,
                                            parse_output(result),
                                        )));
                                    }
                                    Err(err) => {
                                        let _ = tx.send(ProgressMessage::Failed(err.into()));
                                    }
                                };
                            }
                            Ok(None) => {
                                let _ = tx.send(ProgressMessage::Failed(CmdError::CommandNotDone));
                            }
                            Err(err) => {
                                let _ = tx.send(ProgressMessage::Failed(err.into()));
                            }
                        }

                        break;
                    }
                    // Send the next line read from stderr
                    Ok(_) => {
                        let _ = tx.send(ProgressMessage::Update(buffer.clone()));
                    }
                    Err(err) => {
                        let _ = tx.send(ProgressMessage::Failed(err.into()));
                        break;
                    }
                }
            }
        });

        reader
    });

    Ok(context)
}
