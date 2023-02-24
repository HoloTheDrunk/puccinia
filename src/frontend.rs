use std::{
    io::Stdout,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use tui::style::Color;

use crate::grid::Grid;

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
    #[error("Channel error: {0:?}")]
    Channel(mpsc::TryRecvError),
    #[error("Terminal failure: {0:?}")]
    Terminal(std::io::Error),
    #[error("Terminated by logic thread")]
    Terminated,
}

type Result<T> = anyhow::Result<T, Error>;

#[derive(Default, Debug)]
struct State {
    mode: EditorMode,
    grid: Grid,
    tooltip: Option<Tooltip>,
}

#[derive(Default, Debug)]
enum EditorMode {
    #[default]
    Normal,
    Input,
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Tooltip {
    Error(String),
    Help,
}

#[derive(Debug)]
#[allow(unused)]
pub enum Message {
    Break,
    Load(String),
    LogicFail(Option<String>),
    PopupToggle(Tooltip),
}

pub(crate) fn run(receiver: Receiver<Message>) -> Result<()> {
    let mut terminal = setup_terminal().map_err(|err| Error::Terminal(err))?;

    let res = wrapper(&mut terminal, receiver);

    restore_terminal(terminal).map_err(|err| Error::Terminal(err))?;

    res
}

fn wrapper<B: Backend>(terminal: &mut Terminal<B>, receiver: Receiver<Message>) -> Result<()> {
    let mut state = State {
        grid: Grid::new(10, 10),
        ..Default::default()
    };

    main_loop(terminal, &mut state, &receiver)?;

    wait_for_exit().map_err(|err| Error::Terminal(err))?;

    Err(Error::Unknown("Oopsie".to_owned()))
}

fn setup_terminal() -> std::io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    Ok(Terminal::new(backend)?)
}

fn restore_terminal<B: Backend + std::io::Write>(mut terminal: Terminal<B>) -> std::io::Result<()> {
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
) -> Result<()> {
    let mut stop: bool;
    let mut last_frame = Instant::now();

    loop {
        let now = Instant::now();
        let delta = now.duration_since(last_frame);
        last_frame = now;

        stop = handle_events(state)?;

        try_receive_message(state, receiver)?;

        terminal
            .draw(|f| {
                ui(f, state);
            })
            .map_err(|err| Error::Terminal(err))?;

        if stop {
            break;
        }

        if delta < Duration::from_millis(80) {
            std::thread::sleep(Duration::from_millis(80) - delta);
        }
    }

    Ok(())
}

fn try_receive_message(state: &mut State, receiver: &Receiver<Message>) -> Result<()> {
    match receiver.try_recv() {
        Ok(Message::Load(content)) => {
            state.grid = Grid::from(content);
        }
        Ok(Message::Break) => return Err(Error::Terminated),
        Ok(Message::LogicFail(opt_msg)) => {
            state.tooltip = opt_msg.map(|msg| Tooltip::Error(msg));
        }
        Ok(Message::PopupToggle(_)) => todo!(),
        Err(err) => match err {
            TryRecvError::Empty => (),
            TryRecvError::Disconnected => return Err(Error::Channel(err)),
        },
    }

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &mut State) {
    let size = f.size();

    f.render_widget(Block::default().title("MST").borders(Borders::ALL), size);

    f.render_widget(
        state.grid.clone(),
        size.inner(&Margin {
            vertical: 5,
            horizontal: 5,
        }),
    );

    render_tooltip(f, state);
}

fn handle_events(state: &mut State) -> Result<bool> {
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(0)) {
        match crossterm::event::read() {
            Ok(Event::Key(KeyEvent { code, .. })) => match code {
                KeyCode::Char('q') => {
                    state.tooltip = Some(Tooltip::Error("Press 'q' to exit".to_owned()));
                    return Ok(true);
                }
                KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l')) => {
                    if let Err(err) = match c {
                        'h' => state.grid.move_cursor(-1, 0),
                        'j' => state.grid.move_cursor(0, 1),
                        'k' => state.grid.move_cursor(0, -1),
                        'l' => state.grid.move_cursor(1, 0),
                        _ => unreachable!(),
                    } {
                        state.tooltip = Some(Tooltip::Error(format!(
                            "Invalid move (out of bounds): {err:?}"
                        )));
                    }
                }
                _ => todo!(),
            },
            Err(err) => return Err(Error::Terminal(err)),
            _ => (),
        }
    }

    Ok(false)
}

fn render_tooltip<B: Backend>(frame: &mut Frame<B>, state: &State) {
    let size = frame.size();

    if let Some(tooltip) = state.tooltip.clone() {
        match tooltip {
            Tooltip::Help => (),
            Tooltip::Error(err) => {
                let trunc = err.as_str().truncate_ellipse((size.width - 10) as usize);
                frame.render_widget(
                    Paragraph::new(trunc.clone()).style(Style::default().fg(Color::Red)),
                    Rect {
                        x: 0,
                        y: size.bottom() - 1,
                        width: trunc.len() as u16,
                        height: 1,
                    },
                )
            }
        }
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
