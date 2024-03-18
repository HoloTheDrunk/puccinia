use std::{sync::mpsc::Sender, time::Duration};

use crate::{
    cell::{CellValue, Direction},
    logic,
};

use super::prelude::*;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

pub fn handle_events(
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(0)) {
        match crossterm::event::read() {
            Ok(Event::Key(KeyEvent {
                code, modifiers, ..
            })) => {
                let shift = !(modifiers & KeyModifiers::SHIFT).is_empty();
                let ctrl = !(modifiers & KeyModifiers::CONTROL).is_empty();

                match (code, state.mode.clone()) {
                    (KeyCode::Char(':'), EditorMode::Normal | EditorMode::Running) => {
                        state.previous_mode = Some(state.mode.clone());
                        state.mode = EditorMode::Command(String::new());
                    }
                    (KeyCode::Char('h' | 'j' | 'k' | 'l'), EditorMode::Command(_)) if ctrl => (),
                    (KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')), _) if ctrl => match c {
                        'h' => state.grid.pan(Direction::Left),
                        'j' => state.grid.pan(Direction::Down),
                        'k' => state.grid.pan(Direction::Up),
                        'l' => state.grid.pan(Direction::Right),
                        _ => unreachable!(),
                    },
                    _ => match state.mode {
                        EditorMode::Normal => {
                            return handle_events_normal_mode(
                                (code, shift, ctrl),
                                state,
                                interactions,
                                sender,
                            );
                        }
                        EditorMode::Command(ref cmd) => {
                            return handle_events_command_mode(
                                (code, shift, ctrl),
                                cmd.clone(),
                                state,
                                interactions,
                                sender,
                            );
                        }
                        EditorMode::Visual(_, _) => {
                            handle_events_visual_mode((code, shift, ctrl), state, sender)?;
                        }
                        EditorMode::Insert => {
                            handle_events_insert_mode((code, shift, ctrl), state, sender)?;
                        }
                        EditorMode::Running => {
                            handle_events_running_mode((code, shift, ctrl), state, sender)?;
                        }
                    },
                }
            }
            Err(err) => return Err(Error::Terminal(err)),
            _ => (),
        }
    }

    Ok(false)
}

pub fn handle_events_running_mode(
    (code, _shift, ctrl): (KeyCode, bool, bool),
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    match code {
        KeyCode::Esc => {
            state.mode = EditorMode::Normal;
            state.grid.clear_heat();
            sender.send(logic::Message::RunningCommand(logic::RunningCommand::Stop))?;
        }
        KeyCode::Char('c') if ctrl => {
            sender.send(logic::Message::RunningCommand(logic::RunningCommand::Stop))?;
        }
        KeyCode::Char(' ') => {
            sender.send(logic::Message::RunningCommand(logic::RunningCommand::Step))?;
        }
        KeyCode::Char('b') => {
            sender.send(logic::Message::RunningCommand(
                logic::RunningCommand::ToggleBreakpoint,
            ))?;
        }
        KeyCode::Enter => {
            sender.send(logic::Message::RunningCommand(
                logic::RunningCommand::SkipToBreakpoint,
            ))?;
        }
        _ => (),
    }

    Ok(())
}

pub fn handle_events_visual_mode(
    (code, _shift, _ctrl): (KeyCode, bool, bool),
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let EditorMode::Visual(ref mut start, ref mut end) = state.mode else {
        unreachable!()
    };

    match code {
        KeyCode::Char('d') => {
            let (start, end) = (*start, *end);
            copy_area_to_clipboard(start, end, state);

            state
                .grid
                .loop_over((start, end), |_x, _y, cell| cell.value = CellValue::Empty);

            state.mode = EditorMode::Normal;
        }
        KeyCode::Char('y') => {
            let (start, end) = (*start, *end);
            copy_area_to_clipboard(start, end, state);
        }
        KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')) => {
            match c {
                'h' => state.grid.move_cursor(Direction::Left, true, false),
                'j' => state.grid.move_cursor(Direction::Down, true, false),
                'k' => state.grid.move_cursor(Direction::Up, true, false),
                'l' => state.grid.move_cursor(Direction::Right, true, false),
                _ => unreachable!(),
            };

            *end = state.grid.get_cursor();
        }
        KeyCode::Esc => state.mode = EditorMode::Normal,
        _ => (),
    }

    if state.mode == EditorMode::Normal {
        sender.send(logic::Message::Sync(state.grid.dump()))?;
    }

    Ok(())
}

pub fn handle_events_insert_mode(
    (code, _shift, _ctrl): (KeyCode, bool, bool),
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    match code {
        KeyCode::Char(c) => {
            state.grid.set_current(CellValue::from(c));
            state
                .grid
                .move_cursor(state.grid.get_cursor_dir(), true, true);
        }
        KeyCode::Backspace => {
            if !state
                .grid
                .move_cursor(-state.grid.get_cursor_dir(), false, false)
            {
                state.grid.set_current(CellValue::from(' '));
            }
        }
        KeyCode::Delete => {
            state.grid.set_current(CellValue::from(' '));
        }
        KeyCode::Esc => {
            state.mode = EditorMode::Normal;
            sender.send(logic::Message::Sync(state.grid.dump()))?;
        }
        _ => (),
    }

    Ok(())
}

pub fn handle_events_command_mode(
    (code, _shift, _ctrl): (KeyCode, bool, bool),
    mut cmd: String,
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    let exit_command_mode = |state: &mut State| {
        if let Some(mode) = state.previous_mode.as_ref() {
            state.mode = mode.clone();
            state.previous_mode = None;
        } else {
            state.mode = EditorMode::Normal;
        }
    };

    match code {
        KeyCode::Up => {
            if !cmd.trim().is_empty() && state.command_history_index.is_none() {
                state.command_history.push_front(cmd);
            }

            if state.command_history.len() > 0 {
                let new_index = state
                    .command_history_index
                    .take()
                    .map(|index| (index + 1).min(state.command_history.len() - 1))
                    .unwrap_or(0);
                state.command_history_index = Some(new_index);
                state.mode = EditorMode::Command(state.command_history[new_index].clone());
            }
        }
        KeyCode::Down => {
            if state.command_history_index == Some(0) {
                state.command_history_index = None;
                state.mode = EditorMode::Command(String::new());
                return Ok(false);
            }

            let new_index = state
                .command_history_index
                .take()
                .map(|index| index.saturating_sub(1));

            match new_index {
                Some(index) => {
                    state.command_history_index = Some(index);
                    state.mode = EditorMode::Command(state.command_history[index].clone());
                }
                None => (),
            }
        }
        KeyCode::Char(c) => {
            cmd.push(c);
            state.command_history_index = None;
            state.mode = EditorMode::Command(cmd);
        }
        KeyCode::Enter => {
            exit_command_mode(state);
            state.tooltip = None;
            if state.command_history_index.is_none() && !cmd.trim().is_empty() {
                state.command_history.push_front(cmd.clone());
            }
            state.command_history_index = None;
            match handle_command(cmd.as_ref(), state, interactions, sender) {
                Ok(exit) => return Ok(exit),
                Err(err) => state.tooltip = Some(Tooltip::Error(err.to_string())),
            }
        }
        KeyCode::Esc => {
            exit_command_mode(state);
            state.tooltip = None;
        }
        KeyCode::Backspace => {
            cmd.pop();
            state.mode = EditorMode::Command(cmd);
        }
        _ => (),
    }

    Ok(false)
}

fn handle_events_normal_mode(
    (code, _shift, ctrl): (KeyCode, bool, bool),
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    match code {
        KeyCode::Char('i') => {
            state.mode = EditorMode::Insert;
        }
        KeyCode::Char('f') => {
            state.config.run_area_position = state.config.run_area_position.next();
        }
        KeyCode::Char('b') => {
            state.grid.toggle_current_breakpoint();
        }
        KeyCode::Char('v') => {
            let pos = state.grid.get_cursor();
            state.mode = EditorMode::Visual(pos, pos);
        }
        KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')) => {
            match c {
                'h' => state.grid.move_cursor(Direction::Left, true, false),
                'j' => state.grid.move_cursor(Direction::Down, true, false),
                'k' => state.grid.move_cursor(Direction::Up, true, false),
                'l' => state.grid.move_cursor(Direction::Right, true, false),
                _ => unreachable!(),
            };
        }
        KeyCode::Char(c @ ('H' | 'J' | 'K' | 'L')) => {
            match c {
                'H' => state.grid.prepend_column(),
                'J' => state.grid.append_line(None),
                'K' => state.grid.prepend_line(None),
                'L' => state.grid.append_column(),
                _ => unreachable!(),
            };
        }
        KeyCode::Char('p') => {
            let content = match state.clipboard.get_text() {
                Ok(v) => v,
                Err(err) => {
                    state.tooltip = Some(Tooltip::Error(err.to_string()));
                    return Ok(false);
                }
            };
            let c_width = content.lines().map(|line| line.len()).max().unwrap_or(0);
            let c_height = content.lines().count();

            let (x, y) = state.grid.get_cursor();
            let (g_width, g_height) = state.grid.size();

            for _ in g_width..(x + c_width) {
                state.grid.append_column();
            }

            for _ in g_height..(y + c_height) {
                state.grid.append_line(None);
            }

            for (j, line) in content.lines().enumerate() {
                for (i, c) in line.chars().enumerate() {
                    state.grid.set(x + i, y + j, c.into());
                }
            }

            sender.send(logic::Message::Sync(state.grid.dump()))?;
        }
        KeyCode::Char('r') if ctrl => return handle_command("run", state, interactions, sender),
        KeyCode::Esc => state.tooltip = None,
        _ => (),
    }

    Ok(false)
}

fn copy_area_to_clipboard(start: (usize, usize), end: (usize, usize), state: &mut State) {
    let mut block = String::new();

    for y in (start.1.min(end.1))..=(end.1.max(start.1)) {
        for x in (start.0.min(end.0))..=(end.0.max(start.0)) {
            block.push(state.grid.get(x, y).value.into());
        }
        block.push('\n');
    }

    state.mode = EditorMode::Normal;
    if let Err(err) = state.clipboard.set_text(block) {
        state.tooltip = Some(Tooltip::Error(err.to_string()));
    }
}
