use std::sync::mpsc::Sender;

use clap::Parser;

use crate::frontend;

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

type Result<T> = anyhow::Result<T>;

#[derive(Parser)]
/// Minesweeper TUI editor and runner
struct Args {
    /// Input file location
    input: String,
}

pub fn run(sender: Sender<crate::frontend::Message>) -> Result<()> {
    if let Err(err) = load_file(&sender) {
        sender.send(frontend::Message::LogicFail(Some(err.to_string())))?;
    }

    Ok(())
}

fn load_file(sender: &Sender<crate::frontend::Message>) -> Result<()> {
    let Args { input: path } = Args::parse();

    let content = std::fs::read_to_string(&path)
        .map_err(|_| Error::FileError(FileError::FileNotFound(path)))?;

    sender.send(frontend::Message::Load(content))?;

    Ok(())
}
