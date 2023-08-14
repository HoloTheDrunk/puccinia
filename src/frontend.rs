use std::{
    io::{Stdout, Write},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

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

    tooltip: Option<Tooltip>,
    config: Config,
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
    let mut stop: bool;
    let mut last_frame = Instant::now();
    let target_fps = 30;
    let target_delta = Duration::from_millis(1000 / target_fps);

    loop {
        let now = Instant::now();
        let delta = now.duration_since(last_frame);

        if delta > Duration::from_millis(80) {
            std::thread::sleep(target_delta - delta);
        }

        last_frame = now;

        stop = handle_events(state, sender)?;

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
            Message::LeaveRunningMode => state.mode = EditorMode::Normal,
            Message::Output(s) => state.output.push_str(s.as_ref()),
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
        output_area.height = state.config.output_area_height;
        output_area.y = stack_area.bottom() - state.config.output_area_height;
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

    // TODO: Implement grid panning because lmao
    // if state.grid.size().0 * 3 < grid_area.width as usize {
    f.render_stateful_widget(
        state.grid.clone(),
        grid_area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
        &mut (state.mode.clone(), state.config.clone()),
    );
    // } else {
    //     f.render_widget(
    //         Paragraph::new("Too wide, need to implement panning sorry :("),
    //         grid_area.inner(&Margin {
    //             vertical: 1,
    //             horizontal: 1,
    //         }),
    //     );
    // }

    if let EditorMode::Command(ref cmd) = state.mode {
        state.tooltip = Some(Tooltip::Command(cmd.clone()));
    }

    render_tooltip(f, grid_area, state);
}

fn handle_events(state: &mut State, sender: &Sender<logic::Message>) -> AnyResult<bool> {
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(0)) {
        match crossterm::event::read() {
            Ok(Event::Key(KeyEvent { code, .. })) => match (code, state.mode.clone()) {
                (KeyCode::Char(':'), EditorMode::Normal | EditorMode::Running) => {
                    state.previous_mode = Some(state.mode.clone());
                    state.mode = EditorMode::Command(String::new());
                }
                _ => match state.mode {
                    EditorMode::Normal => return handle_events_normal_mode(code, state),
                    EditorMode::Command(ref cmd) => {
                        return handle_events_command_mode(code, cmd.clone(), state, sender)
                    }
                    EditorMode::Insert => {
                        handle_events_insert_mode(code, state, sender)?;
                    }
                    EditorMode::Running => {
                        handle_events_running_mode(code, state, sender)?;
                    }
                },
            },
            Err(err) => return Err(Error::Terminal(err)),
            _ => (),
        }
    }

    Ok(false)
}

fn handle_events_running_mode(
    code: KeyCode,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    match code {
        KeyCode::Esc => {
            state.mode = EditorMode::Normal;
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
    code: KeyCode,
    state: &mut State,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    match code {
        KeyCode::Char(c) => {
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("test.log")
                .unwrap();
            log.write_all(format!("Wrote {c} at {:?}\n", state.grid.get_cursor()).as_bytes())
                .unwrap();
            state.grid.set_current(CellValue::from(c));
            state.grid.move_cursor(state.grid.get_cursor_dir(), true);
        }
        KeyCode::Backspace => {
            if !state.grid.move_cursor(-state.grid.get_cursor_dir(), false) {
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
    code: KeyCode,
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
        KeyCode::Char(c) => {
            cmd.push(c);
            state.mode = EditorMode::Command(cmd);
        }
        KeyCode::Enter => {
            exit_command_mode(state);
            state.tooltip = None;
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
                logic::RunningCommand::Start(state.grid.get_breakpoints()),
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
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let args = args.split(' ').collect::<Vec<&str>>();
    if args.len() != 1 {
        return Err(Error::Command(CommandError::InvalidCommandSyntax));
    }
    let property = args[0];

    match property {
        "lids" => state.config.lids = !state.config.lids,
        "sides" => state.config.sides = !state.config.sides,
        "heat" => state.config.heat = !state.config.heat,
        _ => {
            return Err(Error::Command(CommandError::UnrecognizedProperty(
                property.to_owned(),
            )));
        }
    }

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

        _ => {
            return Err(Error::Command(CommandError::Unknown(
                property.to_owned(),
                args.iter().map(ToString::to_string).collect(),
            )))
        }
    }

    Ok(())
}

fn handle_events_normal_mode(code: KeyCode, state: &mut State) -> AnyResult<bool> {
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
        KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')) => {
            match c {
                'h' => state.grid.move_cursor(Direction::Left, true),
                'j' => state.grid.move_cursor(Direction::Down, true),
                'k' => state.grid.move_cursor(Direction::Up, true),
                'l' => state.grid.move_cursor(Direction::Right, true),
                _ => unreachable!(),
            };
        }
        KeyCode::Right => state.grid.add_column(),
        KeyCode::Down => state.grid.add_line(None),
        KeyCode::Esc => state.tooltip = None,
        _ => (),
    }

    Ok(false)
}

fn render_tooltip<B: Backend>(frame: &mut Frame<B>, area: Rect, state: &State) {
    if let Some(tooltip) = state.tooltip.clone() {
        let (title, content, style) = match tooltip {
            Tooltip::Command(cmd) => ("Command", cmd, Style::default().fg(Color::Yellow)),
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
