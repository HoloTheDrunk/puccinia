use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use crate::{
    cell::{CellValue, Direction, IfDir},
    frontend,
    grid::Grid,
    Args,
};

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
    string_mode: bool,
}

type AnyResult<T> = anyhow::Result<T>;

pub(crate) fn run(
    args: Args,
    sender: Sender<crate::frontend::Message>,
    receiver: Receiver<Message>,
) -> AnyResult<()> {
    let path = args.input.as_str();
    let mut state = State {
        grid: Grid::from(
            std::fs::read_to_string(path)
                .map_err(|_| Error::FileError(FileError::FileNotFound(path.to_owned())))?,
        ),
        stack: Vec::new(),
        string_mode: false,
    };

    update_frontend(&sender, &state)?;

    // Event loop
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Kill => {
                break;
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
                RunningCommand::Step => match step(&sender, &mut state)? {
                    RunStatus::Continue => (),
                    RunStatus::Breakpoint => todo!(),
                    RunStatus::End => sender.send(frontend::Message::LeaveRunningMode)?,
                },
                RunningCommand::SkipToBreakpoint => loop {
                    match step(&sender, &mut state)? {
                        RunStatus::Continue => (),
                        RunStatus::Breakpoint => break,
                        RunStatus::End => {
                            sender.send(frontend::Message::LeaveRunningMode)?;
                            break;
                        }
                    }
                },
            },
        }
    }

    sender.send(frontend::Message::Break)?;

    Ok(())
}

fn update_frontend(sender: &Sender<crate::frontend::Message>, state: &State) -> AnyResult<()> {
    sender.send(frontend::Message::Load((
        state.grid.clone(),
        state.stack.clone(),
    )))?;

    Ok(())
}

enum RunStatus {
    Continue,
    Breakpoint,
    End,
}

/// Run a single step, updating the frontend as required.
fn step(sender: &Sender<crate::frontend::Message>, state: &mut State) -> AnyResult<RunStatus> {
    let cell = state.grid.get_current();

    match cell.value {
        CellValue::Empty => (),

        CellValue::StringMode => state.string_mode = !state.string_mode,

        CellValue::Op(_) => todo!(),
        CellValue::Dir(dir) => state.grid.set_cursor_dir(dir),
        CellValue::If(if_dir) => {
            let (non_zero, zero) = match if_dir {
                IfDir::Horizontal => (Direction::Left, Direction::Right),
                IfDir::Vertical => (Direction::Up, Direction::Down),
            };

            let value = state.stack.pop();
            if value.is_none() || value == Some(0) {
                state.grid.set_cursor_dir(zero);
            } else {
                state.grid.set_cursor_dir(non_zero);
            }
        }

        CellValue::Bridge => {
            // TODO: remove move error, wrap around instead
            if state
                .grid
                .move_cursor(state.grid.get_cursor_dir(), false)
                .is_err()
            {
                ()
            }
        }

        CellValue::Number(num) => state.stack.push(num as i32),
        CellValue::Char(c) => {
            if state.string_mode {
                state.stack.push(c as i32)
            }
        }

        CellValue::End => return Ok(RunStatus::End),
    }

    if state
        .grid
        .move_cursor(state.grid.get_cursor_dir(), false)
        .is_err()
    {
        ()
    };

    // sender.send(frontend::Message::MoveCursor(state.grid.get_cursor()))?;
    update_frontend(sender, state)?;

    Ok(RunStatus::Continue)
}
