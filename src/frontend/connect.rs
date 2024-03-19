use std::sync::mpsc::{Receiver, TryRecvError};

use {
    super::prelude::*,
    crate::{cell::CellValue, grid::Grid},
};

#[derive(Debug)]
#[allow(unused)]
pub enum Message {
    Break,
    MoveCursor((usize, usize)),
    Load((Grid, Vec<i32>, Vec<(usize, usize)>)),
    LogicError(String),
    PopupToggle(Tooltip),
    SetCell { x: usize, y: usize, v: char },
    LeaveRunningMode,
    Output(String),
    Input(InputMode),
}

pub fn try_receive_message(state: &mut State, receiver: &Receiver<Message>) -> AnyResult<()> {
    match receiver.try_recv() {
        Ok(msg) => match msg {
            Message::Load((grid, stack, breakpoints)) => {
                state.grid = Grid::from(grid);
                state.grid.load_breakpoints(breakpoints);
                state.stack = stack;
                state.push_history();
            }
            Message::MoveCursor((x, y)) => {
                state
                    .grid
                    .set_cursor(x, y)
                    .expect("Mismatch between frontend and logic threads' state");
            }
            Message::Break => return Err(Error::Terminated),
            Message::LogicError(msg) => {
                state.tooltip = Some(Tooltip::Error(msg));
            }
            Message::PopupToggle(_) => todo!(),
            Message::SetCell { x, y, v } => state.grid.set(x, y, CellValue::from(v)),
            Message::LeaveRunningMode => {
                state.mode = EditorMode::Normal;
                if !state.config.live_output {
                    state.output = state.output_buffer.take().unwrap_or_else(String::new);
                }
            }
            Message::Output(s) => {
                if state.config.live_output {
                    state.output.push_str(s.as_ref())
                } else {
                    state.output_buffer = Some({
                        let mut current = state.output_buffer.clone().unwrap_or_else(String::new);
                        current.push_str(s.as_ref());
                        current
                    })
                }
            }
            Message::Input(mode) => {
                state.mode = EditorMode::Input(mode, "".to_string());
            }
        },
        Err(err) => match err {
            TryRecvError::Empty => (),
            TryRecvError::Disconnected => return Err(Error::ChannelRecv(err)),
        },
    }

    Ok(())
}
