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
    Start(Vec<(usize, usize)>),
    Step,
    SkipToBreakpoint,
    ToggleBreakpoint,
}

#[derive(Debug, Default)]
struct State {
    grid: Grid,
    stack: Vec<i32>,
    string_mode: bool,
    config: Config,
}

#[derive(Debug)]
struct Config {
    heat_diffusion: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { heat_diffusion: 30 }
    }
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
        ..Default::default()
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
                RunningCommand::Start(breakpoints) => {
                    state.grid.set_cursor(0, 0).unwrap();
                    state.grid.set_cursor_dir(Direction::Right);
                    state.grid.clear_heat();
                    state.grid.clear_breakpoints();
                    state.stack.clear();

                    breakpoints
                        .iter()
                        .for_each(|(x, y)| state.grid.toggle_breakpoint(*x, *y));
                }
                RunningCommand::Step => match step(&sender, &mut state, true)? {
                    RunStatus::Continue => (),
                    RunStatus::Breakpoint => (),
                    RunStatus::End => sender.send(frontend::Message::LeaveRunningMode)?,
                },
                RunningCommand::SkipToBreakpoint => {
                    loop {
                        match step(&sender, &mut state, false)? {
                            RunStatus::Continue => (),
                            RunStatus::Breakpoint => break,
                            RunStatus::End => {
                                sender.send(frontend::Message::LeaveRunningMode)?;
                                break;
                            }
                        }
                    }
                    update_frontend(&sender, &state)?;
                }
                RunningCommand::ToggleBreakpoint => state.grid.toggle_current_breakpoint(),
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
fn step(
    sender: &Sender<crate::frontend::Message>,
    state: &mut State,
    live: bool,
) -> AnyResult<RunStatus> {
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
            state.grid.move_cursor(state.grid.get_cursor_dir(), false);
        }

        CellValue::Number(num) => state.stack.push(num as i32),
        CellValue::Char(c) => {
            if state.string_mode {
                state.stack.push(c as i32)
            }
        }

        CellValue::End => return Ok(RunStatus::End),
    }

    state.grid.reduce_heat(state.config.heat_diffusion);
    state.grid.set_current_heat(128);

    state.grid.move_cursor(state.grid.get_cursor_dir(), false);

    if live {
        update_frontend(sender, state)?;
    }

    Ok(if state.grid.get_current().is_breakpoint {
        RunStatus::Breakpoint
    } else {
        RunStatus::Continue
    })
}
