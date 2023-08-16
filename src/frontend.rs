use std::{
    collections::VecDeque,
    io::{Stdout, Write},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

use crossterm::event::KeyModifiers;
use tui::{style::Color, widgets::Wrap};

use crate::{
    cell::{CellValue, Direction},
    grid::Grid,
    logic,
};

use {
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    ellipse::Ellipse,
    tui::{
        backend::{Backend, CrosstermBackend},
        layout::{Margin, Rect},
        style::Style,
        widgets::{Block, Borders, Paragraph},
        Frame, Terminal,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("Channel receive error: {0:?}")]
    ChannelRecv(#[from] mpsc::TryRecvError),
    #[error("Channel send error: {0:?}")]
    ChannelSend(#[from] mpsc::SendError<logic::Message>),
    #[error("Terminal failure: {0:?}")]
    Terminal(#[from] std::io::Error),
    #[error("Terminated by logic thread")]
    Terminated,
    #[error("{0}")]
    Command(CommandError),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("{0}")]
    Unique(String),
    #[error("{0:?} must be UTF-8")]
    EncodingError(CommandPart),
    #[error("Unrecognized property: {0}")]
    UnrecognizedProperty(String),
    #[error("Invalid arguments: {0:?}")]
    InvalidArguments(Vec<String>),
    #[error("Usage: set <property> [values...]")]
    InvalidCommandSyntax,
    #[error("Invalid command or number of paremeters: {0} {1:?}")]
    Unknown(String, Vec<String>),
}

#[derive(Debug)]
pub enum CommandPart {
    Property,
    Arguments,
}

type AnyResult<T> = anyhow::Result<T, Error>;

#[derive(Clone, Default, Debug)]
pub struct Config {
    // Side area for run information
    pub run_area_width: u16,
    pub run_area_position: RunAreaPosition,
    pub output_area_height: u16,

    // Editor display settings
    pub heat: bool,
    pub lids: bool,
    pub sides: bool,

    // Running mode optimizations
    pub live_output: bool,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum RunAreaPosition {
    #[default]
    Left,
    Right,
    Hidden,
}

impl RunAreaPosition {
    pub fn next(&self) -> Self {
        match self {
            RunAreaPosition::Left => RunAreaPosition::Right,
            RunAreaPosition::Right => RunAreaPosition::Hidden,
            RunAreaPosition::Hidden => RunAreaPosition::Left,
        }
    }
}

#[derive(Default, Debug)]
struct State {
    mode: EditorMode,
    previous_mode: Option<EditorMode>,

    grid: Grid,
    stack: Vec<i32>,
    output: String,
    output_buffer: Option<String>,

    tooltip: Option<Tooltip>,
    config: Config,

    command_history: VecDeque<String>,
    command_history_index: Option<usize>,

    debug: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum EditorMode {
    #[default]
    /// Mode for moving around efficiently and running commands
    Normal,
    /// Command input mode
    Command(String),
    /// Text edition mode
    Insert,
    /// Running state
    Running,
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Tooltip {
    Command(String),
    Info(String),
    Error(String),
}

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
}

pub(crate) fn run(receiver: Receiver<Message>, sender: Sender<logic::Message>) -> AnyResult<()> {
    let mut terminal = setup_terminal()?;

    let res = wrapper(&mut terminal, receiver, &sender);

    restore_terminal(terminal, &sender)?;

    res
}

fn wrapper<B: Backend>(
    terminal: &mut Terminal<B>,
    receiver: Receiver<Message>,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let mut state = State {
        grid: Grid::new(10, 10),
        config: Config {
            run_area_width: 32,
            run_area_position: RunAreaPosition::Left,
            output_area_height: 24,

            heat: true,
            lids: true,
            sides: true,

            live_output: true,
        },
        ..Default::default()
    };

    main_loop(terminal, &mut state, &receiver, &sender)?;

    Ok(())
}

fn setup_terminal() -> std::io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    Ok(Terminal::new(backend)?)
}

fn restore_terminal<B: Backend + std::io::Write>(
    mut terminal: Terminal<B>,
    sender: &Sender<logic::Message>,
) -> std::io::Result<()> {
    if sender.send(logic::Message::Kill).is_err() {
        // Ignore already killed logic thread
    }

    disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}

fn main_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut State,
    receiver: &Receiver<Message>,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let mut last_frame = Instant::now();
    let target_fps = 30;
    let target_delta = Duration::from_millis(1000 / target_fps);

    loop {
        let start = Instant::now();
        let delta = start - last_frame;

        if delta < target_delta {
            std::thread::sleep(target_delta - delta);
        }

        last_frame = Instant::now();

        let stop = handle_events(state, sender)?;

        try_receive_message(state, receiver)?;

        terminal.draw(|f| {
            ui(f, state);
        })?;

        if stop {
            break;
        }
    }

    Ok(())
}

fn try_receive_message(state: &mut State, receiver: &Receiver<Message>) -> AnyResult<()> {
    match receiver.try_recv() {
        Ok(msg) => match msg {
            Message::Load((grid, stack, breakpoints)) => {
                state.grid = Grid::from(grid);
                state.grid.load_breakpoints(breakpoints);
                state.stack = stack;
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
        },
        Err(err) => match err {
            TryRecvError::Empty => (),
            TryRecvError::Disconnected => return Err(Error::ChannelRecv(err)),
        },
    }

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &mut State) {
    let frame_size = f.size();

    let mut grid_area = frame_size.clone();
    let mut stack_area = frame_size.clone();

    let is_debug = state.debug.is_some();

    // Don't render the run area if the terminal is too thin
    if state.config.run_area_position != RunAreaPosition::Hidden
        && frame_size.width > state.config.run_area_width
    {
        grid_area.width -= state.config.run_area_width;
        stack_area.width = state.config.run_area_width;

        if state.config.run_area_position == RunAreaPosition::Right {
            stack_area.x = grid_area.width;
        } else {
            grid_area.x += state.config.run_area_width;
        }

        let mut output_area = stack_area.clone();
        output_area.height = state.config.output_area_height - 3 * is_debug as u16;
        output_area.y = stack_area.bottom() - state.config.output_area_height + 3 * is_debug as u16;
        stack_area.height -= state.config.output_area_height;

        f.render_widget(
            Block::default().title("Stack").borders(Borders::ALL),
            stack_area,
        );

        f.render_widget(
            Paragraph::new(
                state
                    .stack
                    .iter()
                    .map(|v| v.to_string())
                    .rev()
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
            stack_area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            }),
        );

        if is_debug {
            let debug_area = Rect::new(stack_area.left(), stack_area.bottom(), stack_area.width, 3);

            f.render_widget(
                Block::default()
                    .title("Debug")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::LightGreen)),
                debug_area,
            );

            f.render_widget(
                Paragraph::new(state.debug.clone().unwrap_or(" ".to_owned())),
                debug_area.inner(&Margin {
                    vertical: 1,
                    horizontal: 2,
                }),
            );
        }

        f.render_widget(
            Block::default().title("Output").borders(Borders::ALL),
            output_area,
        );

        f.render_widget(
            Paragraph::new(
                state
                    .output
                    .lines()
                    // Might be needed if wrapping doesn't work nicely enough
                    // .map(|line| {
                    //     line.truncate_ellipse((output_area.width - 1) as usize)
                    //         .to_string()
                    // })
                    .collect::<Vec<&str>>()
                    .join("\n"),
            )
            .wrap(Wrap { trim: false }),
            output_area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            }),
        );
    }

    f.render_widget(
        Block::default()
            .title("Editor")
            .borders(Borders::ALL)
            .style(Style::default().fg(match state.mode {
                EditorMode::Normal => Color::White,
                EditorMode::Command(_) => Color::DarkGray,
                EditorMode::Insert => Color::Yellow,
                EditorMode::Running => Color::Red,
            })),
        grid_area,
    );

    f.render_stateful_widget(
        state.grid.clone(),
        grid_area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
        &mut (state.mode.clone(), state.config.clone()),
    );

    if let EditorMode::Command(ref cmd) = state.mode {
        state.tooltip = Some(Tooltip::Command(cmd.clone()));
    }

    render_tooltip(f, grid_area, state);
}

fn handle_events(state: &mut State, sender: &Sender<logic::Message>) -> AnyResult<bool> {
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
                            return handle_events_normal_mode((code, shift, ctrl), state, sender);
                        }
                        EditorMode::Command(ref cmd) => {
                            return handle_events_command_mode(
                                (code, shift, ctrl),
                                cmd.clone(),
                                state,
                                sender,
                            );
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

fn handle_events_running_mode(
    (code, shift, ctrl): (KeyCode, bool, bool),
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    match code {
        KeyCode::Esc => {
            state.mode = EditorMode::Normal;
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

fn handle_events_insert_mode(
    (code, shift, ctrl): (KeyCode, bool, bool),
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

fn handle_events_command_mode(
    (code, shift, ctrl): (KeyCode, bool, bool),
    mut cmd: String,
    state: &mut State,
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
            match handle_command(cmd.as_ref(), state, sender) {
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

// Returns whether or not the program should exit due to a fatal error.
fn handle_command(
    cmd: &str,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    match cmd.trim().as_bytes() {
        b"" => (),
        b"q" => return Ok(true),
        [b'w', path @ ..] => {
            let path = String::from_utf8(path.to_vec())
                .map_err(|_| Error::Command(CommandError::EncodingError(CommandPart::Arguments)))?;
            sender
                .send(logic::Message::Write(
                    (!path.trim().is_empty()).then(|| path.trim().to_owned()),
                ))
                .unwrap();
        }
        b"trim" => {
            let trimmed = state.grid.trim();

            state.tooltip = Some(Tooltip::Info(format!("{trimmed:?}")));

            if trimmed.iter().any(|v| *v != 0) && !state.grid.check_bounds(state.grid.get_cursor())
            {
                state.grid.set_cursor(0, 0).unwrap();
            }
        }
        b"run" => {
            state.grid.set_cursor(0, 0).unwrap();
            state.grid.set_cursor_dir(Direction::Right);
            state.grid.clear_heat();

            state.stack = Vec::new();
            state.output = String::new();

            state.mode = EditorMode::Running;

            if state.config.run_area_position == RunAreaPosition::Hidden {
                state.config.run_area_position = RunAreaPosition::Left;
            }

            sender.send(logic::Message::RunningCommand(
                logic::RunningCommand::Start(state.grid.dump(), state.grid.get_breakpoints()),
            ))?;
        }
        [b's', b'e', b't', args @ ..] => {
            let args = String::from_utf8(args.to_vec())
                .map_err(|_| Error::Command(CommandError::EncodingError(CommandPart::Arguments)))?;
            handle_set_command(args.trim(), state, sender)?;
        }
        [b't', b'o', b'g', b'g', b'l', b'e', args @ ..] => {
            let args = String::from_utf8(args.to_vec())
                .map_err(|_| Error::Command(CommandError::EncodingError(CommandPart::Arguments)))?;
            handle_toggle_command(args.trim(), state, sender)?;
        }
        _ => state.tooltip = Some(Tooltip::Error(format!("Unknown command `{cmd}`"))),
    }

    Ok(false)
}

fn handle_toggle_command(
    args: &str,
    state: &mut State,
    _sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let args = args.split(' ').collect::<Vec<&str>>();
    if args.len() != 1 {
        return Err(Error::Command(CommandError::InvalidCommandSyntax));
    }
    let property = args[0];

    let result = match property {
        "lids" => {
            state.config.lids = !state.config.lids;
            state.config.lids
        }
        "sides" => {
            state.config.sides = !state.config.sides;
            state.config.sides
        }
        "heat" => {
            state.config.heat = !state.config.heat;
            state.config.heat
        }
        "live_output" => {
            if state.mode == EditorMode::Running {
                state.tooltip = Some(Tooltip::Error(
                    "Can't change output mode during a run".to_owned(),
                ));
                return Ok(());
            } else {
                state.config.live_output = !state.config.live_output;
                state.config.live_output
            }
        }
        _ => {
            return Err(Error::Command(CommandError::UnrecognizedProperty(
                property.to_owned(),
            )));
        }
    };

    state.tooltip = Some(Tooltip::Info(format!(
        "Turned {property} {}",
        if result { "on" } else { "off" }
    )));

    Ok(())
}

fn handle_set_command(
    args: &str,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let args = args.split(' ').collect::<Vec<&str>>();
    if args.len() < 1 {
        return Err(Error::Command(CommandError::InvalidCommandSyntax));
    }

    let property = args[0];
    let args = &args[1..];

    match (property, args.len()) {
        ("lids", 1) => state.grid.lids = args[0].chars().next().unwrap(),

        ("sides", 1) => state.grid.sides = args[0].chars().next().unwrap(),

        ("heat_diffusion", 1) => sender.send(logic::Message::UpdateProperty(
            property.to_owned(),
            args[0].to_owned(),
        ))?,

        ("view_updates", 1) => sender.send(logic::Message::UpdateProperty(
            property.to_owned(),
            args[0].to_owned(),
        ))?,

        ("step_ms", 1) => sender.send(logic::Message::UpdateProperty(
            property.to_owned(),
            args[0].to_owned(),
        ))?,

        _ => {
            return Err(Error::Command(CommandError::Unknown(
                property.to_owned(),
                args.iter().map(ToString::to_string).collect(),
            )))
        }
    }

    state.tooltip = Some(Tooltip::Info(format!(
        "Set {property} to {}",
        args.join(" ")
    )));

    Ok(())
}

fn handle_events_normal_mode(
    (code, _shift, ctrl): (KeyCode, bool, bool),
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    match (code, ctrl) {
        (KeyCode::Char('i'), false) => {
            state.mode = EditorMode::Insert;
        }
        (KeyCode::Char('f'), false) => {
            state.config.run_area_position = state.config.run_area_position.next();
        }
        (KeyCode::Char('b'), false) => {
            state.grid.toggle_current_breakpoint();
        }
        (KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')), false) => {
            match c {
                'h' => state.grid.move_cursor(Direction::Left, true, false),
                'j' => state.grid.move_cursor(Direction::Down, true, false),
                'k' => state.grid.move_cursor(Direction::Up, true, false),
                'l' => state.grid.move_cursor(Direction::Right, true, false),
                _ => unreachable!(),
            };
        }
        (KeyCode::Char(c @ ('H' | 'J' | 'K' | 'L')), false) => {
            match c {
                'H' => state.grid.prepend_column(),
                'J' => state.grid.append_line(None),
                'K' => state.grid.prepend_line(None),
                'L' => state.grid.append_column(),
                _ => unreachable!(),
            };
        }
        (KeyCode::Char('r'), true) => return handle_command("run", state, sender),
        (KeyCode::Esc, false) => state.tooltip = None,
        _ => (),
    }

    Ok(false)
}

fn render_tooltip<B: Backend>(frame: &mut Frame<B>, area: Rect, state: &State) {
    if let Some(tooltip) = state.tooltip.clone() {
        let (title, content, style) = match tooltip {
            Tooltip::Command(cmd) => ("Command", cmd, Style::default().fg(Color::Yellow)),
            Tooltip::Info(info) => ("Info", info, Style::default().fg(Color::Green)),
            Tooltip::Error(err) => ("Error", err, Style::default().fg(Color::Red)),
        };

        let trunc = content
            .as_str()
            .truncate_ellipse((area.width - 10) as usize);

        let command_area = Rect {
            x: area.left(),
            y: area.bottom() - 3,
            width: (trunc.len() as u16).max(title.len() as u16) + 4,
            height: 3,
        };

        frame.render_widget(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(style),
            command_area,
        );

        frame.render_widget(
            Paragraph::new(trunc.clone()).style(style),
            command_area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            }),
        );
    }
}
