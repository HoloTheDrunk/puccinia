use std::{
    io::Stdout,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use tui::{style::Color, widgets::canvas::Rectangle};

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
        text::Text,
        widgets::{Block, Borders, Paragraph, Widget},
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
}

type Result<T> = anyhow::Result<T, Error>;

#[derive(Debug)]
#[allow(unused)]
pub enum Message {
    Break,
    Load(String),
    LogicFail(Option<String>),
    PopupToggle(Tooltip),
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Tooltip {
    Error(String),
    Help,
}

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
    let mut stop = false;
    loop {
        match receiver.try_recv() {
            Ok(Message::Load(content)) => {
                state.grid = Grid::from(content);
            }
            Ok(Message::Break) => break,
            Ok(Message::LogicFail(opt_msg)) => {
                state.tooltip = opt_msg.map(|msg| Tooltip::Error(msg));
            }
            Ok(Message::PopupToggle(_)) => todo!(),
            Err(err) => match err {
                TryRecvError::Empty => (),
                TryRecvError::Disconnected => return Err(Error::Channel(err)),
            },
        }

        if let Ok(true) = crossterm::event::poll(Duration::from_millis(0)) {
            match crossterm::event::read() {
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                })) => {
                    state.tooltip = Some(Tooltip::Error("Press 'q' to exit".to_owned()));
                    stop = true;
                }
                Err(err) => return Err(Error::Terminal(err)),
                _ => (),
            }
        }

        terminal
            .draw(|f| {
                let size = f.size();

                f.render_widget(Block::default().title("MST").borders(Borders::ALL), size);

                f.render_widget(
                    state.grid.clone(),
                    size.inner(&Margin {
                        vertical: 5,
                        horizontal: 5,
                    }),
                );

                print_tooltip(f, state);
            })
            .map_err(|err| Error::Terminal(err))?;

        if stop {
            break;
        }
    }

    Ok(())
}

fn print_tooltip<B: Backend>(frame: &mut Frame<B>, state: &State) {
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
