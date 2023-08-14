use std::{
    io::Write,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use crate::{
    cell::{
        BinaryOperator, CellValue, Direction, IfDir, NullaryOperator, Operator, TernaryOperator,
        UnaryOperator,
    },
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
    // std::fs::OpenOptions::new().create(true).open(path)?;
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
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("test.log")?;

    let cell = state.grid.get_current();

    match cell.value {
        CellValue::Empty => (),

        CellValue::StringMode => state.string_mode = !state.string_mode,

        CellValue::Op(op) => match op {
            Operator::Nullary(op) => match op {
                NullaryOperator::Integer => todo!(),
                NullaryOperator::Ascii => todo!(),
            },
            Operator::Unary(op) => {
                let popped = state.stack.pop().unwrap_or(0);
                match op {
                    UnaryOperator::Negate => state.stack.push(if popped == 0 { 1 } else { 0 }),
                    UnaryOperator::Duplicate => {
                        state.stack.push(popped);
                        state.stack.push(popped);
                    }
                    UnaryOperator::Pop => (),
                    UnaryOperator::WriteNumber => {
                        sender.send(frontend::Message::Output(popped.to_string()))?;
                    }
                    UnaryOperator::WriteASCII => sender.send(frontend::Message::Output(
                        String::from_utf8([popped as u8].to_vec())?,
                    ))?,
                }
            }
            Operator::Binary(op) => {
                let b = state.stack.pop().unwrap_or(0);
                let a = state.stack.pop().unwrap_or(0);
                match op {
                    BinaryOperator::Greater => state.stack.push((a > b) as i32),
                    BinaryOperator::Add => state.stack.push(a + b),
                    BinaryOperator::Subtract => state.stack.push(a - b),
                    BinaryOperator::Multiply => state.stack.push(a * b),
                    BinaryOperator::Divide => state.stack.push(if b != 0 { a / b } else { 0 }),
                    BinaryOperator::Modulo => state.stack.push(if b != 0 { a % b } else { 0 }),
                    BinaryOperator::Swap => {
                        state.stack.push(b);
                        state.stack.push(a);
                    }
                    BinaryOperator::Get => {
                        let (width, height) = state.grid.size();
                        if a < 0 || b < 0 || a > width as i32 || b > height as i32 {
                            state.stack.push(0);
                        } else {
                            state.stack.push(char::from(
                                state.grid.get(a as usize, b as usize).value,
                            ) as i32);
                        }
                    }
                }
            }
            Operator::Ternary(op) => {
                let y = state.stack.pop().unwrap_or(0);
                let x = state.stack.pop().unwrap_or(0);
                let v = state.stack.pop().unwrap_or(0);
                match op {
                    TernaryOperator::Put => {
                        let (width, height) = state.grid.size();
                        if !(x < 0 || y < 0 || x > width as i32 || y > height as i32) {
                            state.grid.set(
                                x as usize,
                                y as usize,
                                char::from_u32(v as u32).unwrap().into(),
                            );
                        }
                    }
                }
            }
        },

        CellValue::Dir(dir) => state.grid.set_cursor_dir(dir),
        CellValue::If(if_dir) => {
            log.write_all(b"Going through IF")?;

            let (non_zero, zero) = match if_dir {
                IfDir::Horizontal => (Direction::Left, Direction::Right),
                IfDir::Vertical => (Direction::Up, Direction::Down),
            };

            let value = state.stack.pop().unwrap_or(0);
            if value == 0 {
                log.write_all(format!("Going {:?}", zero).as_bytes())?;
                state.grid.set_cursor_dir(zero);
            } else {
                log.write_all(format!("Going {:?}", non_zero).as_bytes())?;
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

    log.write_all(format!("Went {:?}", state.grid.get_cursor_dir()).as_bytes())?;
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
