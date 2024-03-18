mod command;
mod connect;
mod input;
mod state;

use std::{
    collections::VecDeque,
    io::Stdout,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use {
    crate::{cell::Direction, grid::Grid, logic},
    command::*,
    connect::*,
    input::*,
    state::*,
};

use {
    arboard::Clipboard,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    itertools::Itertools,
    tui::{
        backend::{Backend, CrosstermBackend},
        layout::{Margin, Rect},
        style::{Color, Style},
        widgets::Wrap,
        widgets::{Block, Borders, Paragraph},
        Frame, Terminal,
    },
};

pub mod prelude {
    pub use super::{command::*, connect::*, input::*, state::*, *};
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
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
    #[error("Clipboard error: {0}")]
    Clipboard(#[from] arboard::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("Unrecognized property: {0}")]
    UnrecognizedProperty(String),
    #[error("Invalid arguments: {0:?}")]
    InvalidArguments(Vec<String>),
    #[error("Usage: set <property> [values...]")]
    InvalidCommandSyntax,
    #[error("Invalid command or number of paremeters: {0} {1:?}")]
    Unknown(String, Vec<String>),
}

type AnyResult<T> = anyhow::Result<T, Error>;

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
        mode: EditorMode::Normal,
        previous_mode: None,
        stack: Vec::new(),
        output: String::new(),
        output_buffer: None,
        tooltip: None,
        command_history: VecDeque::new(),
        command_history_index: None,
        clipboard: Clipboard::new()?,
        debug: None,
    };

    // Keeping them separate for simplicity's sake as commands need to mutably borrow the state.
    let interactions = Interactions {
        commands: init_commands(),
        properties: init_properties(),
    };

    main_loop(terminal, &mut state, interactions, &receiver, &sender)?;

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
    interactions: Interactions,
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

        let stop = handle_events(state, &interactions, sender)?;

        connect::try_receive_message(state, receiver)?;

        terminal.draw(|f| {
            ui(f, state);
        })?;

        if stop {
            break;
        }
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
                EditorMode::Visual(_, _) => Color::Cyan,
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
        state,
    );

    if let EditorMode::Command(ref cmd) = state.mode {
        state.tooltip = Some(Tooltip::Command(cmd.clone()));
    }

    render_tooltip(f, grid_area, state);
}

fn render_tooltip<B: Backend>(frame: &mut Frame<B>, area: Rect, state: &State) {
    if let Some(tooltip) = state.tooltip.clone() {
        let (title, content, style) = match tooltip {
            Tooltip::Command(cmd) => ("Command", cmd, Style::default().fg(Color::Yellow)),
            Tooltip::Info(info) => ("Info", info, Style::default().fg(Color::Green)),
            Tooltip::Error(err) => ("Error", err, Style::default().fg(Color::Red)),
        };

        let lines = content
            .lines()
            .map(str::trim)
            .flat_map(|s| {
                s.chars()
                    .chunks(area.width as usize - 10)
                    .into_iter()
                    .map(|chunk| chunk.collect::<String>())
                    .collect::<Vec<String>>()
            })
            .collect::<Vec<String>>();

        let command_area = Rect {
            x: area.left(),
            y: area.bottom() - 2 - lines.len().max(1) as u16,
            width: (lines.iter().map(String::len).max().unwrap_or(0) as u16)
                .max(title.len() as u16)
                + 4,
            height: lines.len().max(1) as u16 + 2,
        };

        frame.render_widget(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(style),
            command_area,
        );

        frame.render_widget(
            Paragraph::new(lines.join("\n").clone()).style(style),
            command_area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            }),
        );
    }
}
