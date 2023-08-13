use std::{
    io::Stdout,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

use tui::style::Color;

use crate::{
    cell::{CellValue, Direction},
    grid::Grid,
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
    ChannelRecv(mpsc::TryRecvError),
    #[error("Channel send error: {0:?}")]
    ChannelSend(mpsc::SendError<crate::logic::Message>),
    #[error("Terminal failure: {0:?}")]
    Terminal(std::io::Error),
    #[error("Terminated by logic thread")]
    Terminated,
}

type Result<T> = anyhow::Result<T, Error>;

#[derive(Default, Debug)]
struct Config {
    run_area_width: u16,
    run_area_on_right: bool,
    output_area_height: u16,
}

#[derive(Default, Debug)]
struct State {
    mode: EditorMode,
    previous_mode: Option<EditorMode>,

    grid: Grid,
    stack: Vec<i32>,

    tooltip: Option<Tooltip>,
    config: Config,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
enum EditorMode {
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
    Load((Grid, Vec<i32>)),
    LogicFail(Option<String>),
    PopupToggle(Tooltip),
    SetCell { x: usize, y: usize, v: char },
    LeaveRunningMode,
}

pub(crate) fn run(
    receiver: Receiver<Message>,
    sender: Sender<crate::logic::Message>,
) -> Result<()> {
    let mut terminal = setup_terminal().map_err(|err| Error::Terminal(err))?;

    let res = wrapper(&mut terminal, receiver, &sender);

    restore_terminal(terminal, &sender).map_err(|err| Error::Terminal(err))?;

    res
}

fn wrapper<B: Backend>(
    terminal: &mut Terminal<B>,
    receiver: Receiver<Message>,
    sender: &Sender<crate::logic::Message>,
) -> Result<()> {
    let mut state = State {
        grid: Grid::new(10, 10),
        config: Config {
            run_area_width: 32,
            run_area_on_right: false,
            output_area_height: 32,
        },
        ..Default::default()
    };

    main_loop(terminal, &mut state, &receiver, &sender)?;

    // wait_for_exit().map_err(|err| Error::Terminal(err))?;

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
    sender: &Sender<crate::logic::Message>,
) -> std::io::Result<()> {
    sender.send(crate::logic::Message::Kill).unwrap();

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
    sender: &Sender<crate::logic::Message>,
) -> Result<()> {
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

        terminal
            .draw(|f| {
                ui(f, state);
            })
            .map_err(|err| Error::Terminal(err))?;

        if stop {
            break;
        }
    }

    Ok(())
}

fn try_receive_message(state: &mut State, receiver: &Receiver<Message>) -> Result<()> {
    match receiver.try_recv() {
        Ok(msg) => match msg {
            Message::Load((grid, stack)) => {
                state.grid = Grid::from(grid);
                state.stack = stack;
            }
            Message::MoveCursor((x, y)) => {
                state
                    .grid
                    .set_cursor(x, y)
                    .expect("Mismatch between frontend and logic threads' state");
            }
            Message::Break => return Err(Error::Terminated),
            Message::LogicFail(opt_msg) => {
                state.tooltip = opt_msg.map(|msg| Tooltip::Error(msg));
            }
            Message::PopupToggle(_) => todo!(),
            Message::SetCell { x, y, v } => state.grid.set(x, y, CellValue::from(v)),
            Message::LeaveRunningMode => state.mode = EditorMode::Normal,
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

    // Don't render the stack area if the terminal is too thin
    if frame_size.width > state.config.run_area_width {
        grid_area.width -= state.config.run_area_width;
        stack_area.width = state.config.run_area_width;

        if state.config.run_area_on_right {
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
            Block::default().title("Output").borders(Borders::ALL),
            output_area,
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

    f.render_widget(
        state.grid.clone(),
        grid_area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
    );

    if let EditorMode::Command(ref cmd) = state.mode {
        state.tooltip = Some(Tooltip::Command(cmd.clone()));
    }

    render_tooltip(f, grid_area, state);
}

fn handle_events(state: &mut State, sender: &Sender<crate::logic::Message>) -> Result<bool> {
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
                        handle_events_running_mode(code, state);
                    }
                },
            },
            Err(err) => return Err(Error::Terminal(err)),
            _ => (),
        }
    }

    Ok(false)
}

fn handle_events_running_mode(code: KeyCode, state: &mut State) {
    match code {
        KeyCode::Esc => {
            state.mode = EditorMode::Normal;
        }
        _ => (),
    }
}

fn handle_events_insert_mode(
    code: KeyCode,
    state: &mut State,
    sender: &Sender<crate::logic::Message>,
) -> Result<()> {
    match code {
        KeyCode::Char(c) => {
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
            sender
                .send(crate::logic::Message::Sync(state.grid.dump()))
                .map_err(|err| Error::ChannelSend(err))?;
        }
        _ => (),
    }

    Ok(())
}

fn handle_events_command_mode(
    code: KeyCode,
    mut cmd: String,
    state: &mut State,
    sender: &Sender<crate::logic::Message>,
) -> Result<bool> {
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
            return handle_command(cmd.as_ref(), state, sender);
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
    sender: &Sender<crate::logic::Message>,
) -> Result<bool> {
    match cmd.trim().as_bytes() {
        b"" => (),
        b"q" => return Ok(true),
        [b'w', path @ ..] => {
            let path = String::from_utf8(path.to_vec()).expect("Provided write path is not UTF-8");
            sender
                .send(crate::logic::Message::Write(
                    (!path.trim().is_empty()).then(|| path.trim().to_owned()),
                ))
                .unwrap();
        }
        b"run" => state.mode = EditorMode::Running,
        _ => state.tooltip = Some(Tooltip::Error(format!("Unknown command `{cmd}`"))),
    }

    Ok(false)
}

fn handle_events_normal_mode(code: KeyCode, state: &mut State) -> Result<bool> {
    match code {
        KeyCode::Char('i') => {
            state.mode = EditorMode::Insert;
        }
        KeyCode::Char('f') => {
            state.config.run_area_on_right = !state.config.run_area_on_right;
        }
        KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')) => {
            match c {
                'h' => state.grid.move_cursor(Direction::Left, true),
                'j' => state.grid.move_cursor(Direction::Down, true),
                'k' => state.grid.move_cursor(Direction::Up, true),
                'l' => state.grid.move_cursor(Direction::Right, true),
                _ => unreachable!(),
            };
            // } {
            //     state.tooltip = Some(Tooltip::Error(format!("Invalid move destination: {err:?}")));
            // }
        }
        KeyCode::Esc => state.tooltip = None,
        _ => (),
    }

    Ok(false)
}

fn update_mode(mode: EditorMode, state: &mut State) {
    if let Some(mode) = state.previous_mode.as_ref() {
        state.mode = mode.clone();
        state.previous_mode = None;
    } else {
        state.mode = EditorMode::Normal;
    }
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
            width: (trunc.len() as u16).max(title.len() as u16) + 2,
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
                horizontal: 1,
            }),
        );
    }
}

fn wait_for_exit() -> std::io::Result<()> {
    loop {
        match crossterm::event::read() {
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
            })) => break,
            Err(err) => return Err(err),
            _ => (),
        }
    }

    Ok(())
}
