use std::{sync::mpsc::Sender, time::Duration};

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

type Result<T> = anyhow::Result<T>;

pub(crate) fn run(args: Args, sender: Sender<crate::frontend::Message>) -> Result<()> {
    if let Err(err) = load_file(args.input.as_str(), &sender) {
        sender.send(frontend::Message::LogicFail(Some(err.to_string())))?;
    }

    std::thread::sleep(Duration::from_secs(2));

    Ok(())
}

fn load_file(path: &str, sender: &Sender<crate::frontend::Message>) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| Error::FileError(FileError::FileNotFound(path.to_owned())))?;

    sender.send(frontend::Message::Load(content.to_owned()))?;

    Ok(())
}
