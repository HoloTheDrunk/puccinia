use crate::{
    cell::{
        BinaryOperator, CellValue, Direction, IfDir, NullaryOperator, Operator, TernaryOperator,
        UnaryOperator,
    },
    frontend::prelude::{InputMode, Message as FMessage},
    grid::Grid,
    Args,
};

use std::{
    path::Path,
    str::FromStr,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

use strum::{EnumString, EnumVariantNames, VariantNames};

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
    UpdateProperty(String, String),
    Input(i32),
}

#[derive(Debug)]
pub enum RunningCommand {
    Start(String, Vec<(usize, usize)>),
    Step,
    SkipToBreakpoint,
    ToggleBreakpoint,
    Stop,
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
    view_updates: ViewUpdates,
    heat_diffusion: u8,
    step_ms: u64,
}

#[derive(Clone, Copy, Debug, EnumString, EnumVariantNames, PartialEq, Eq)]
#[strum(ascii_case_insensitive)]
enum ViewUpdates {
    None,
    Partial,
    All,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            view_updates: ViewUpdates::All,
            heat_diffusion: 30,
            step_ms: 80,
        }
    }
}

type AnyResult<T> = anyhow::Result<T>;

pub(crate) fn run(
    args: Args,
    sender: Sender<FMessage>,
    receiver: Receiver<Message>,
) -> AnyResult<()> {
    let path = args.input.as_str();

    let mut state = State {
        grid: if Path::new(path).is_file() {
            Grid::from(
                std::fs::read_to_string(path)
                    .map_err(|_| Error::FileError(FileError::FileNotFound(path.to_owned())))?,
            )
        } else {
            Grid::default()
        },
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
                let mut to_save = state.grid.clone();
                to_save.trim();
                std::fs::write(path, to_save.dump())?;
            }
            Message::Write(None) => std::fs::write(path, state.grid.dump())?,
            Message::Sync(grid) => {
                state.grid = Grid::from(grid);
            }
            Message::RunningCommand(command) => match command {
                RunningCommand::Start(grid, breakpoints) => {
                    state.grid.load_values(grid);

                    state.grid.set_cursor(0, 0).unwrap();
                    state.grid.set_cursor_dir(Direction::Right);

                    state.grid.clear_heat();
                    state.grid.clear_breakpoints();

                    state.stack.clear();

                    breakpoints
                        .iter()
                        .for_each(|(x, y)| state.grid.toggle_breakpoint(*x, *y));
                }
                RunningCommand::Step => match step(&sender, &receiver, &mut state, true)? {
                    RunStatus::Continue => (),
                    RunStatus::Breakpoint => (),
                    RunStatus::End => sender.send(FMessage::LeaveRunningMode)?,
                },
                RunningCommand::SkipToBreakpoint => {
                    loop {
                        let start = Instant::now();

                        match step(&sender, &receiver, &mut state, false)? {
                            RunStatus::Continue => (),
                            RunStatus::Breakpoint => break,
                            RunStatus::End => {
                                sender.send(FMessage::LeaveRunningMode)?;
                                break;
                            }
                        }

                        if let Ok(Message::RunningCommand(RunningCommand::Stop)) =
                            receiver.try_recv()
                        {
                            sender.send(FMessage::LeaveRunningMode)?;
                            break;
                        }

                        if state.config.view_updates == ViewUpdates::All && state.config.step_ms > 10 {
                            let end = Instant::now();
                            let delta = end - start;

                            if delta < Duration::from_millis(state.config.step_ms) {
                                std::thread::sleep(Duration::from_millis(
                                    state.config.step_ms - delta.as_millis() as u64,
                                ));
                            }
                        }
                    }
                    update_frontend(&sender, &state)?;
                }
                RunningCommand::ToggleBreakpoint => state.grid.toggle_current_breakpoint(),
                RunningCommand::Stop => (),
            },
            Message::UpdateProperty(property, value) => match property.as_ref() {
                "heat_diffusion" => match value.parse() {
                    Ok(heat_diffusion) => state.config.heat_diffusion = heat_diffusion,
                    Err(_) => sender.send(FMessage::LogicError(format!(
                        "Failed to parse `{value}` to u8; valid values are from 0 to 255 included."
                    )))?,
                },
                "view_updates" => match ViewUpdates::from_str(value.as_ref()) {
                    Ok(vu) => state.config.view_updates = vu,
                    Err(_) => sender.send(FMessage::LogicError(format!(
                        "Unrecognized ViewUpdates variant {}, valid variants are {:?}",
                        value,
                        ViewUpdates::VARIANTS
                    )))?,
                },
                "step_ms" => match value.parse() {
                    Ok(step_ms) => state.config.step_ms = step_ms,
                    Err(_) => sender.send(FMessage::LogicError(format!(
                        "Failed to parse `{value}` to u64; valid values are from 0 to <big> included."
                    )))?,
                }
                _ => sender.send(FMessage::LogicError(format!(
                    "Unrecognized property `{property}`",
                )))?,
            },
            Message::Input(value) => {
                sender.send(FMessage::LogicError(format!("Unexpected input at this time: {value}")))?
            }
        }
    }

    sender.send(FMessage::Break)?;

    Ok(())
}

// TODO: Add a lightweight version of this based on sending only change events
// This is the biggest bottleneck for the interpreter right now
fn update_frontend(sender: &Sender<FMessage>, state: &State) -> AnyResult<()> {
    sender.send(FMessage::Load((
        state.grid.clone(),
        state.stack.clone(),
        state.grid.get_breakpoints(),
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
    sender: &Sender<FMessage>,
    receiver: &Receiver<Message>,
    state: &mut State,
    live: bool,
) -> AnyResult<RunStatus> {
    let cell = state.grid.get_current();

    let mut grid_update = false;

    match cell.value {
        CellValue::StringMode => state.string_mode = !state.string_mode,

        _ if state.string_mode => state.stack.push(char::from(cell.value) as i32),

        CellValue::Empty => (),

        CellValue::Op(op) => match op {
            Operator::Nullary(op) => match op {
                NullaryOperator::Integer | NullaryOperator::Ascii => {
                    if op == NullaryOperator::Integer {
                        sender.send(FMessage::Input(InputMode::Integer))?;
                    } else {
                        sender.send(FMessage::Input(InputMode::ASCII))?;
                    }

                    let Message::Input(value) = receiver.recv()? else {
                        sender.send(FMessage::LogicError("Expected input".to_string()))?;
                        sender.send(FMessage::LeaveRunningMode)?;
                        return Ok(RunStatus::End);
                    };

                    state.stack.push(value);
                }
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
                        sender.send(FMessage::Output(popped.to_string()))?;
                    }
                    UnaryOperator::WriteASCII => sender.send(FMessage::Output(
                        String::from_utf8([popped.rem_euclid(u8::MAX as i32 + 1) as u8].to_vec())?,
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
                            grid_update = true;
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
            let (non_zero, zero) = match if_dir {
                IfDir::Horizontal => (Direction::Left, Direction::Right),
                IfDir::Vertical => (Direction::Up, Direction::Down),
            };

            let value = state.stack.pop().unwrap_or(0);
            if value == 0 {
                state.grid.set_cursor_dir(zero);
            } else {
                state.grid.set_cursor_dir(non_zero);
            }
        }

        CellValue::Bridge => {
            state.grid.set_current_heat(128);
            state
                .grid
                .move_cursor(state.grid.get_cursor_dir(), false, false);
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

    state
        .grid
        .move_cursor(state.grid.get_cursor_dir(), false, false);

    if live {
        update_frontend(sender, state)?;
    } else {
        match (state.config.view_updates, grid_update) {
            (ViewUpdates::All, _) | (ViewUpdates::Partial, true) => update_frontend(sender, state)?,
            _ => (),
        }
    }

    Ok(if state.grid.get_current().is_breakpoint {
        RunStatus::Breakpoint
    } else {
        RunStatus::Continue
    })
}
