use super::prelude::*;

use crate::cell::CellValue;

use {
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers},
    field_iterator::Iterable,
};

#[derive(Clone, Debug, Iterable)]
pub struct Keybindings {
    pub escape: KeyEvent,
    pub cycle_layout: KeyEvent,

    // Deletion
    pub backspace: KeyEvent,
    pub delete_current: KeyEvent,
    pub delete_word: KeyEvent,

    // Cursor movement
    pub left: KeyEvent,
    pub down: KeyEvent,
    pub up: KeyEvent,
    pub right: KeyEvent,

    // Grid expansion
    pub expand_left: KeyEvent,
    pub expand_down: KeyEvent,
    pub expand_up: KeyEvent,
    pub expand_right: KeyEvent,

    // Grid panning
    pub pan_left: KeyEvent,
    pub pan_down: KeyEvent,
    pub pan_up: KeyEvent,
    pub pan_right: KeyEvent,

    // Mode switch
    pub start_command: KeyEvent,
    pub start_insert: KeyEvent,
    pub start_visual: KeyEvent,
    pub start_running: KeyEvent,
    pub start_history: KeyEvent,

    // Normal mode
    pub paste: KeyEvent,

    // Command mode
    pub submit_command: KeyEvent,

    // Visual mode
    pub yank: KeyEvent,
    pub delete: KeyEvent,

    // Running mode
    pub stop_run: KeyEvent,
    pub step: KeyEvent,
    pub skip_to_breakpoint: KeyEvent,

    // Input mode
    pub submit_input: KeyEvent,

    // History mode
    pub history_back: KeyEvent,
    pub history_forward: KeyEvent,
    pub history_accept: KeyEvent,

    // Normal and Running mode
    pub toggle_breakpoint: KeyEvent,
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            // Generics
            escape: KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            cycle_layout: KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),

            // Deletion
            backspace: KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            delete_current: KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            delete_word: KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),

            // Cursor movement
            left: KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            down: KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            up: KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            right: KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),

            // Grid expansion
            expand_left: KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE),
            expand_down: KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE),
            expand_up: KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE),
            expand_right: KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE),

            // Grid panning
            pan_left: KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            pan_down: KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            pan_up: KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            pan_right: KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),

            // Mode switch
            start_command: KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE),
            start_insert: KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            start_visual: KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
            start_running: KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            start_history: KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),

            // Normal mode
            paste: KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),

            // Command mode
            submit_command: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),

            // Visual mode
            yank: KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
            delete: KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),

            // Running mode
            stop_run: KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            step: KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            skip_to_breakpoint: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),

            // Input mode
            submit_input: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),

            // History mode
            history_back: KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
            history_forward: KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            history_accept: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),

            // Normal and Running mode
            toggle_breakpoint: KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        }
    }
}

impl Keybindings {
    pub fn load(raw: &super::ConfigFile) -> anyhow::Result<Self> {
        let mut keybindings = Keybindings::default();

        let mut invalid = Vec::new();

        for (bind, provided_key) in raw.binds.iter() {
            if let Some((_name, current_key)) = keybindings
                .iter_mut()
                .find(|(name, _key)| name == &bind.to_lowercase().as_str())
            {
                *current_key.downcast_mut::<KeyEvent>().unwrap() =
                    parse_key_event(provided_key.as_str())?;
            } else {
                invalid.push(bind.to_string());
            }
        }

        if invalid.is_empty() {
            Ok(keybindings)
        } else {
            Err(InputError::UnknownBinds(invalid).into())
        }
    }

    pub fn movement(&self) -> [&KeyEvent; 4] {
        [&self.left, &self.down, &self.up, &self.right]
    }

    pub fn expansion(&self) -> [&KeyEvent; 4] {
        [
            &self.expand_left,
            &self.expand_down,
            &self.expand_up,
            &self.expand_right,
        ]
    }

    pub fn panning(&self) -> [&KeyEvent; 4] {
        [
            &self.pan_left,
            &self.pan_down,
            &self.pan_up,
            &self.pan_right,
        ]
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InputError {
    #[error("Unknown bind: `{0}`")]
    UnknownBind(String),
    #[error("Unknown binds: {0:?}")]
    UnknownBinds(Vec<String>),
}

fn parse_key_event(key_event: &str) -> anyhow::Result<KeyEvent> {
    let (modifiers, key) = {
        let mut chars = key_event.chars();
        let mut modifiers = KeyModifiers::NONE;
        if key_event.len() > 1 && &key_event[..1] == "^" {
            modifiers |= KeyModifiers::CONTROL;
            chars.next();
        }
        (modifiers, chars.as_str())
    };

    let code = match key {
        "<Space>" => KeyCode::Char(' '),
        "<Enter>" => KeyCode::Enter,
        "<Escape>" => KeyCode::Esc,
        "<Backspace>" => KeyCode::Backspace,
        "<Delete>" => KeyCode::Delete,
        "<Tab>" => KeyCode::Tab,
        "<Insert>" => KeyCode::Insert,
        "<Home>" => KeyCode::Home,
        "<End>" => KeyCode::End,
        "<PageUp>" => KeyCode::PageUp,
        "<PageDown>" => KeyCode::PageDown,
        "<Up>" => KeyCode::Up,
        "<Down>" => KeyCode::Down,
        "<Left>" => KeyCode::Left,
        "<Right>" => KeyCode::Right,
        _ if key.len() == 1 => {
            let code = key.chars().next().unwrap();
            KeyCode::Char(code)
        }
        _ => return Err(InputError::UnknownBind(key.to_string()).into()),
    };

    Ok(KeyEvent::new(code, modifiers))
}

macro_rules! map_binding {
    ($event:expr, $state:expr, match {
        $($($bind:ident)|* $(if $cond:expr)? => $action:expr,)*
        _ => $catchall:expr $(,)?
    }) => {
        'fake_match: { $(
            if $($state.config.bindings.$bind == $event)||* $(&& $cond)? {
                break 'fake_match ($action);
            })*
            $catchall
        }
    }
}

pub fn handle_events(
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> FrontendResult<bool> {
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(0)) {
        match crossterm::event::read() {
            Ok(Event::Key(event)) => {
                match &state.mode {
                    // Shared and guards
                    EditorMode::Normal | EditorMode::Visual(_, _) | EditorMode::Running
                        if event == state.config.bindings.start_command =>
                    {
                        state.previous_mode = Some(state.mode.clone());
                        state.mode = EditorMode::Command(String::new());
                    }
                    EditorMode::Command(_)
                        if state.config.bindings.expansion().contains(&&event) =>
                    {
                        ()
                    }
                    EditorMode::Normal | EditorMode::Running
                        if state.config.bindings.toggle_breakpoint == event =>
                    {
                        if state.mode == EditorMode::Normal {
                            state.grid.toggle_current_breakpoint();
                        } else {
                            sender.send(logic::Message::RunningCommand(
                                logic::RunningCommand::ToggleBreakpoint,
                            ))?;
                        }
                    }
                    _ if state.config.bindings.panning().contains(&&event) => {
                        let dir = ((state.config.bindings.pan_left.code == event.code) as u8) << 3
                            | ((state.config.bindings.pan_down.code == event.code) as u8) << 2
                            | ((state.config.bindings.pan_up.code == event.code) as u8) << 1
                            | (state.config.bindings.pan_right.code == event.code) as u8;

                        match dir {
                            0b1000 => state.grid.pan(Direction::Left),
                            0b0100 => state.grid.pan(Direction::Down),
                            0b0010 => state.grid.pan(Direction::Up),
                            0b0001 => state.grid.pan(Direction::Right),
                            _ => unreachable!(),
                        }
                    }
                    // Dispatch
                    EditorMode::Normal => {
                        return handle_events_normal_mode(event, state, interactions, sender);
                    }
                    EditorMode::Command(ref cmd) => {
                        return handle_events_command_mode(
                            event,
                            cmd.clone(),
                            state,
                            interactions,
                            sender,
                        );
                    }
                    EditorMode::Visual(_, _) => {
                        handle_events_visual_mode(event, state, sender)?;
                    }
                    EditorMode::Insert => {
                        handle_events_insert_mode(event, state, sender)?;
                    }
                    EditorMode::Running => {
                        handle_events_running_mode(event, state, sender)?;
                    }
                    EditorMode::Input(mode, ref string) => {
                        handle_events_input_mode(
                            event,
                            mode.clone(),
                            string.clone(),
                            state,
                            sender,
                        )?;
                    }
                    EditorMode::History(hindex) => {
                        handle_events_history_mode(event, *hindex, state)?;
                    }
                }
            }
            Err(err) => return Err(Error::Terminal(err)),
            _ => (),
        }
    }

    Ok(false)
}

pub fn handle_events_history_mode(
    event: KeyEvent,
    hindex: usize,
    state: &mut State,
) -> FrontendResult<()> {
    map_binding!(event, state, match {
        history_back => {
            let new_index = (hindex + 1).min(state.history.inner.len());
            state.mode = EditorMode::History(new_index);
            state.load_history(new_index);
        },
        history_forward => {
            let new_index = hindex.saturating_sub(1);
            state.mode = EditorMode::History(new_index);
            state.load_history(new_index);
        },
        history_accept => {
            state.mode = EditorMode::Normal;
            state
                .history
                .inner
                .truncate(state.history.inner.len() - hindex);
        },
        escape => {
            state.mode = EditorMode::Normal;
            state.load_history(0);
        },
        _ => (),
    });

    Ok(())
}

pub fn handle_events_input_mode(
    event: KeyEvent,
    input_mode: InputMode,
    mut string: String,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> FrontendResult<()> {
    map_binding!(event, state, match {
        backspace => {
            string.pop();
            state.mode = EditorMode::Input(input_mode, string.clone());
        },
        delete_word => {
            string = string
                .rfind(char::is_whitespace)
                .map(|i| string[..i].to_owned())
                .unwrap_or("".to_owned());
            state.mode = EditorMode::Input(input_mode, string.clone());
        },
        submit_input if string.len() > 0 => {
            let value = match input_mode {
                InputMode::Integer => string
                    .parse::<i32>()
                    .map_err(|_| Error::Input(input_mode, string.clone()))?,
                InputMode::ASCII => string.as_bytes()[0] as i32,
            };

            sender.send(logic::Message::Input(value))?;
            state.mode = EditorMode::Running;
        },
        escape => sender.send(logic::Message::RunningCommand(logic::RunningCommand::Stop))?,
        _ => match event.code {
            KeyCode::Char(c)
                if input_mode == InputMode::Integer
                    && (c.is_digit(10) || (c == '-' && string.len() == 0)) =>
            {
                string.push(c);
                state.mode = EditorMode::Input(input_mode, string);
            }
            KeyCode::Char(c) if input_mode == InputMode::ASCII && c.is_ascii() && string.len() == 0 => {
                string.push(c);
                state.mode = EditorMode::Input(input_mode, string);
            }
            _ => (),
        },
    });

    Ok(())
}

pub fn handle_events_running_mode(
    event: KeyEvent,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> FrontendResult<()> {
    map_binding!(event, state, match {
        stop_run | escape => {
            state.mode = EditorMode::Normal;
            state.grid.clear_heat();
            sender.send(logic::Message::RunningCommand(logic::RunningCommand::Stop))?;
        },
        step => {
            sender.send(logic::Message::RunningCommand(logic::RunningCommand::Step))?;
        },
        skip_to_breakpoint => {
            sender.send(logic::Message::RunningCommand(
                logic::RunningCommand::SkipToBreakpoint,
            ))?;
        },
        _ => (),
    });

    Ok(())
}

pub fn handle_events_visual_mode(
    event: KeyEvent,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> FrontendResult<()> {
    let EditorMode::Visual(start, end) = state.mode else {
        unreachable!()
    };

    map_binding!(event, state, match {
        delete => {
            copy_area_to_clipboard(start, end, state);

            state.push_history();
            state
                .grid
                .loop_over_hv((start, end), |_x, _y, cell| cell.value = CellValue::Empty);
            state.push_history();

            state.mode = EditorMode::Normal;
        },
        yank => {
            copy_area_to_clipboard(start, end, state);
        },
        left | down | up | right => {
            let direction = map_binding!(event, state, match {
                left => Direction::Left,
                down => Direction::Down,
                up => Direction::Up,
                right => Direction::Right,
                _ => unreachable!(),
            });
            state.grid.move_cursor(direction, true, false);

            state.mode = EditorMode::Visual(start, state.grid.get_cursor());
        },
        escape => state.mode = EditorMode::Normal,
        _ => (),
    });

    if state.mode == EditorMode::Normal {
        sender.send(logic::Message::Sync(state.grid.dump()))?;
    }

    Ok(())
}

pub fn handle_events_insert_mode(
    event: KeyEvent,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> FrontendResult<()> {
    map_binding!(event, state, match {
        delete_current if !(event.modifiers & KeyModifiers::SHIFT).is_empty() => {
            state.grid.set_current(CellValue::from(' '));
        },
        backspace => {
            if !state
                .grid
                .move_cursor(-state.grid.get_cursor_dir(), false, false)
            {
                state.grid.set_current(CellValue::from(' '));
            }
        },
        escape => {
            // Only snapshot once per edit session to avoid history cluttering
            state.push_history();

            state.mode = EditorMode::Normal;
            sender.send(logic::Message::Sync(state.grid.dump()))?;
        },
        _ => if let KeyCode::Char(c) = event.code {
            state.grid.set_current(CellValue::from(c));
            state
                .grid
                .move_cursor(state.grid.get_cursor_dir(), true, true);
        },
    });

    Ok(())
}

pub fn handle_events_command_mode(
    event: KeyEvent,
    mut cmd: String,
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> FrontendResult<bool> {
    let exit_command_mode = |state: &mut State| {
        state.mode = state.previous_mode.clone().unwrap_or(EditorMode::Normal);
    };

    map_binding!(event, state, match {
        up => {
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
        },
        down => {
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
        },
        submit_command => {
            exit_command_mode(state);
            state.tooltip = None;
            if state.command_history_index.is_none() && !cmd.trim().is_empty() {
                state.command_history.push_front(cmd.clone());
            }
            state.command_history_index = None;
            let command_result = handle_command(cmd.as_ref(), state, interactions, sender);
            state.previous_mode = None;
            match command_result {
                Ok(exit) => return Ok(exit),
                Err(err) => state.tooltip = Some(Tooltip::Error(err.to_string())),
            }
        },
        escape => {
            exit_command_mode(state);
            state.previous_mode = None;
            state.tooltip = None;
        },
        backspace => {
            cmd.pop();
            state.mode = EditorMode::Command(cmd);
        },
        _ => if let KeyCode::Char(c) = event.code {
            cmd.push(c);
            state.command_history_index = None;
            state.mode = EditorMode::Command(cmd);
        },
    });

    Ok(false)
}

fn handle_events_normal_mode(
    event: KeyEvent,
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> FrontendResult<bool> {
    map_binding!(event, state, match {
        start_insert => {
            state.mode = EditorMode::Insert;
        },
        start_visual => {
            let pos = state.grid.get_cursor();
            state.mode = EditorMode::Visual(pos, pos);
        },
        start_history => {
            state.push_history();
            state.load_history(0);
            state.mode = EditorMode::History(0);
        },
        left | down | up | right => {
            let direction = map_binding!(event, state, match {
                left => Direction::Left,
                down => Direction::Down,
                up => Direction::Up,
                right => Direction::Right,
                _ => unreachable!(),
            });

            state.grid.move_cursor(direction, true, false);
        },
        expand_left | expand_down | expand_up | expand_right => {
            map_binding!(event, state, match {
                expand_left => state.grid.prepend_column(),
                expand_down => state.grid.append_line(None),
                expand_up => state.grid.prepend_line(None),
                expand_right => state.grid.append_column(),
                _ => unreachable!(),
            });
        },
        cycle_layout => {
            state.config.run_area_position = state.config.run_area_position.next();
        },
        paste => {
            let content = match state.clipboard.get_text() {
                Ok(v) => v,
                Err(err) => {
                    state.tooltip = Some(Tooltip::Error(err.to_string()));
                    return Ok(false);
                }
            };

            state.push_history();

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
        },
        escape => state.tooltip = None,
        // Has to be here to avoid generating labeled break return warning
        _ => if event == state.config.bindings.start_running {
            return handle_command("run", state, interactions, sender)
        },
    });

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
