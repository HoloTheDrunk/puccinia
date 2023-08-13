use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use crate::{cell::CellValue, frontend, grid::Grid, Args};

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
    Kill,
    /// Synchronize grid status with frontend
    GetGrid,
    /// Set value at pos
    SetCell {
        x: usize,
        y: usize,
        v: char,
    },
    Sync(String),
    Write(Option<String>),
    RunningCommand(RunningCommand),
}

#[derive(Debug)]
pub enum RunningCommand {
    Start,
    Step,
    SkipToBreakpoint,
}

#[derive(Debug)]
struct State {
    grid: Grid,
    stack: Vec<i32>,
}

type Result<T> = anyhow::Result<T>;

pub(crate) fn run(
    args: Args,
    sender: Sender<crate::frontend::Message>,
    receiver: Receiver<Message>,
) -> Result<()> {
    let path = args.input.as_str();
    let mut state = State {
        grid: Grid::from(
            std::fs::read_to_string(path)
                .map_err(|_| Error::FileError(FileError::FileNotFound(path.to_owned())))?,
        ),
        stack: Vec::new(),
    };

    sender.send(frontend::Message::Load(state.grid.clone()))?;

    // Event loop
    let mut exit = false;
    while !exit {
        // Handle all queued events
        while let Ok(message) = receiver.try_recv() {
            match message {
                Message::Kill => {
                    exit = true;
                    break;
                }
                Message::GetGrid => {
                    sender.send(frontend::Message::Break)?;
                }
                Message::SetCell { x, y, v } => state.grid.set(x, y, CellValue::from(v)),
                Message::Write(Some(path)) => {
                    std::fs::write(path, state.grid.dump())?;
                }
                Message::Write(None) => std::fs::write(path, state.grid.dump())?,
                Message::Sync(grid) => {
                    state.grid = Grid::from(grid);
                }
                Message::RunningCommand(command) => match command {
                    RunningCommand::Start => state.stack.clear(),
                    RunningCommand::Step => todo!(),
                    RunningCommand::SkipToBreakpoint => todo!(),
                },
            }
        }
    }

    sender.send(frontend::Message::Break)?;

    Ok(())
}
