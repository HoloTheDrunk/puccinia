use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use crate::{frontend, Args};

#[derive(thiserror::Error, Clone, Debug)]
#[allow(unused)]
pub enum Error {
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("Load error: {0:?}")]
    FileError(FileError),
}

#[derive(Clone, Debug)]
pub enum FileError {
    FileNotFound(String),
}

#[derive(Debug)]
#[allow(unused)]
pub enum Message {
    /// Synchronize grid status with frontend
    Resync,
    /// Set value at pos
    Set { x: usize, y: usize, v: char },
    /// Get value at pos
    Get { x: usize, y: usize },
}

type Result<T> = anyhow::Result<T>;

pub(crate) fn run(
    args: Args,
    sender: Sender<crate::frontend::Message>,
    receiver: Receiver<Message>,
) -> Result<()> {
    if let Err(err) = load_file(args.input.as_str(), &sender) {
        sender.send(frontend::Message::LogicFail(Some(err.to_string())))?;
    }

    // TODO: replace with actual logic
    std::thread::sleep(Duration::from_secs(10));
    sender.send(frontend::Message::Break)?;

    Ok(())
}

fn load_file(path: &str, sender: &Sender<crate::frontend::Message>) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| Error::FileError(FileError::FileNotFound(path.to_owned())))?;

    sender.send(frontend::Message::Load(content.to_owned()))?;

    Ok(())
}
