mod command;
mod config;
mod connect;
mod input;
mod popup;
mod state;

use std::{
    collections::VecDeque,
    fmt::Debug,
    io::Stdout,
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

use chrono::{DateTime, Local, Utc};
use ratatui::{
    style::Stylize,
    text::{Line, Text},
    widgets::BorderType,
};

use self::popup::{Notification, NotificationContent, Tooltip};

use {
    crate::{cell::Direction, grid::Grid, logic},
    command::*,
    config::ConfigFile,
    connect::*,
    input::*,
    state::*,
};

use {
    ansi_to_tui::IntoText,
    arboard::Clipboard,
    chrono::{Datelike as _, Timelike as _},
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    itertools::Itertools,
    ratatui::{
        backend::{Backend, CrosstermBackend},
        layout::{Margin, Rect},
        style::{Color, Style},
        widgets::Wrap,
        widgets::{Block, Borders, Paragraph},
        Frame, Terminal,
    },
    rune::{termcolor::Buffer, ContextError, Diagnostics, Module},
};

pub mod prelude {
    pub use super::{command::*, connect::*, popup::*, state::*, *};
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
    #[error("Input error: `{1}` is not {0:?}")]
    Input(InputMode, String),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("Unrecognized property: `{0}`")]
    UnrecognizedProperty(String),
    #[error("Invalid arguments: {0:?}")]
    InvalidArguments(Vec<String>),
    #[error("Invalid mode, expected {0} mode")]
    InvalidMode(String),
}

type FrontendResult<T> = anyhow::Result<T, Error>;

pub(crate) fn run(
    receiver: Receiver<Message>,
    sender: Sender<logic::Message>,
) -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;

    let res = wrapper(&mut terminal, receiver, &sender);

    restore_terminal(terminal, &sender)?;

    res
}

fn wrapper<B: Backend>(
    terminal: &mut Terminal<B>,
    receiver: Receiver<Message>,
    sender: &Sender<logic::Message>,
) -> anyhow::Result<()> {
    let load_start = std::time::Instant::now();

    let mut errors = Vec::new();
    let mut log_error = |msg: &str, err: &anyhow::Error| errors.push(format!("{msg}: {err:?}"));

    let (path, expanded_path, config_file) = {
        let puccinia_config_path = std::env::var("PUCCINIA_CONFIG_PATH");
        let dir_path = puccinia_config_path
            .or(std::env::var("HOME"))
            .unwrap_or(".".to_owned());
        let expanded_path = shellexpand::tilde(&dir_path).to_string();
        let config_path = format!("{}/config.pucci", expanded_path);

        (
            dir_path.clone(),
            expanded_path.clone(),
            ConfigFile::try_from(Path::new(config_path.as_str())).ok(),
        )
    };

    let (config_path, bindings) = match config_file.as_ref().map(Keybindings::load).transpose() {
        Ok(opt) => (Some(Path::new(expanded_path.as_str())), opt),
        Err(err) => {
            log_error("Failed to load keybindings", &err);
            (None, None)
        }
    };

    let mut state = State {
        grid: Grid::new(10, 10),
        history: GridHistory::new(256),
        config: Config {
            run_area_width: 32,
            run_area_position: RunAreaPosition::Left,
            output_area_height: 24,

            heat: true,
            lids: true,
            sides: true,

            notification_max_size: (50, 5),
            notification_capacity: 64,
            notification_time: 15,

            live_output: true,

            bindings: bindings.unwrap_or_default(),

            cache_directory: ".cache".to_owned(),
            cache_directory_size: 2,
            autosave_interval: 60,
        },
        mode: EditorMode::Normal,
        previous_mode: None,
        stack: Vec::new(),
        output: String::new(),
        output_buffer: None,
        tooltip: None,
        notifications: Vec::new(),
        command_history: VecDeque::new(),
        command_history_index: None,
        clipboard: Clipboard::new()?,
        debug: None,
        rune_state: None,
    };

    if let Ok(mut sources) = load_rune_commands(config_path).inspect_err(|err| {
        state
            .notifications
            .push(Notification::new(NotificationContent::Info(format!(
                "Custom command loading error: {err:?}"
            ))))
    }) {
        let command_api_module = build_command_api_module()?;

        let mut context = rune_modules::default_context()?;
        context
            .install(command_api_module)
            .expect("Failed to install Rune API module");

        let mut diagnostics = Diagnostics::new();

        let result = rune::prepare(&mut sources)
            .with_context(&context)
            .with_diagnostics(&mut diagnostics)
            .build();

        if !diagnostics.is_empty() {
            let mut buffer = Buffer::no_color();
            diagnostics
                .emit(&mut buffer, &sources)
                .expect("Failed to emit Rune diagnostics");
            state
                .notifications
                .push(Notification::new(NotificationContent::RuneError(
                    String::from_utf8_lossy(buffer.as_slice()).to_string(),
                )));
        } else {
            state.rune_state = Some(RuneState::new(
                context.runtime().expect("Failed to build Rune runtime"),
                result.expect("Failed to build Rune unit"),
            ));
        }
    }

    state
        .notifications
        .push(Notification::new(NotificationContent::Info(
            "This is a notification".to_owned(),
        )));

    state
        .notifications
        .push(Notification::new(NotificationContent::Info(
            "This is another notification".to_owned(),
        )));

    // Keeping them separate for simplicity's sake as commands need to mutably borrow the state.
    let interactions = Interactions {
        commands: init_commands(&config_file, &mut state),
        properties: init_properties(),
    };

    match config_file {
        // Apply properties from the config file
        Some(content) => {
            let errors = content
                .sets
                .iter()
                .flat_map(|(name, args)| {
                    let args = std::iter::once(name.to_owned())
                        .chain(args.split_whitespace().map(str::to_owned))
                        .collect::<Vec<String>>();

                    if let Err(e) =
                        handle_set_command(args.as_slice(), &mut state, &interactions, sender, true)
                    {
                        Some(format!("Failed to apply config: {}", e))
                    } else {
                        None
                    }
                })
                .collect::<Vec<String>>();

            if errors.len() > 0 {
                state.tooltip = Some(Tooltip::Error(errors.join("\n")));
            } else {
                state.tooltip = Some(Tooltip::Info(format!(
                    "Loaded config from {path} in {}ms",
                    (std::time::Instant::now() - load_start).as_millis()
                )));
            }

            for error in errors {
                state
                    .notifications
                    .push(Notification::new(NotificationContent::Error(Box::new(
                        error,
                    ))));
            }
        }
        None => state.tooltip = Some(Tooltip::Info("Loaded default config".to_owned())),
    };

    main_loop(terminal, &mut state, interactions, &receiver, &sender)?;

    Ok(())
}

fn build_command_api_module() -> Result<Module, ContextError> {
    let mut module = Module::new();

    macro_rules! export_types {
        ($($ty:ty),* $(,)?) => {
            $(module.ty::<$ty>()?;)*
        }
    }

    export_types! {
        State,
        Config,
        Grid,
        GridHistory,
        RuneCommandError,
        ArgType,
        Tooltip,
        EditorMode,
        InputMode,
    };

    Ok(module)
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
) -> FrontendResult<()> {
    let mut last_frame = Instant::now();
    let target_fps = 30;
    let target_delta = Duration::from_millis(1000 / target_fps);

    let mut last_autosave = Instant::now();

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

        autosave(state, &mut last_autosave);
    }

    Ok(())
}

fn autosave(state: &mut State, last_autosave: &mut Instant) {
    if state.config.autosave_interval > 0
        && (Instant::now() - *last_autosave).as_secs() > state.config.autosave_interval
    {
        if let Err(err) = std::fs::create_dir_all(state.config.cache_directory.as_str()) {
            state.tooltip = Some(Tooltip::Error(format!(
                "Failed to create cache directory: {}",
                err
            )));
            state.config.autosave_interval = 0;
            return;
        }

        let dir_iter = std::fs::read_dir(state.config.cache_directory.as_str()).unwrap();
        let dir_size = dir_iter
            .map(|entry| {
                entry.map_or(0, |file| {
                    file.metadata()
                        .expect("couldn't access file metadata")
                        .len()
                })
            })
            .reduce(|acc, e| acc + e)
            .unwrap_or(0);

        let dir_iter = std::fs::read_dir(state.config.cache_directory.as_str()).unwrap();
        if dir_size > state.config.cache_directory_size {
            dir_iter
                .sorted_by_key(|entry| {
                    entry.as_ref().map_or(0, |file| {
                        file.metadata()
                            .unwrap()
                            .created()
                            .unwrap()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_else(|_| Duration::from_secs(0))
                            .as_secs()
                    })
                })
                .take(1)
                .for_each(|entry| {
                    if let Ok(entry) = entry {
                        std::fs::remove_file(entry.path()).unwrap();
                    }
                })
        }

        let now = chrono::Local::now();
        let path = Path::new(state.config.cache_directory.as_str()).join(format!(
            "autosave_{}-{}-{}_{}-{}-{}.befunge",
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        ));

        match std::fs::write(path, state.grid.dump()) {
            Ok(_) => *last_autosave = Instant::now(),
            Err(err) => {
                state.tooltip = Some(Tooltip::Error(format!(
                    "Failed to autosave, disabling autosave routine: {err}"
                )));
            }
        }
    }
}

fn ui(f: &mut Frame, state: &mut State) {
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
            .style(Style::default().fg(Color::from(&state.mode))),
        grid_area,
    );

    f.render_widget(
        Paragraph::new(match state.grid.get_cursor_dir() {
            Direction::Up => "^",
            Direction::Down => "v",
            Direction::Left => "<",
            Direction::Right => ">",
            Direction::Random => unreachable!(),
        }),
        Rect::new(
            grid_area.left() + "Editor".len() as u16 + 2,
            grid_area.top(),
            1,
            1,
        ),
    );

    f.render_stateful_widget(
        state.grid.clone(),
        grid_area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
        state,
    );

    match &state.mode {
        EditorMode::Command(cmd) => state.tooltip = Some(Tooltip::Command(cmd.clone())),
        EditorMode::Input(mode, input) => {
            state.tooltip = Some(Tooltip::Input(mode.clone(), input.clone()))
        }
        _ => (),
    }
    if let EditorMode::Command(ref cmd) = state.mode {
        state.tooltip = Some(Tooltip::Command(cmd.clone()));
    }

    render_tooltip(f, grid_area, state);
    render_notifications(f, grid_area, state);
}

fn render_notifications(frame: &mut Frame, grid_area: Rect, state: &mut State) {
    state.debug = Some(format!("Notifications: {}", state.notifications.len()));

    let (max_width, max_height) = state.config.notification_max_size;
    let mut cum_height = 0; // °u°
    let now = Local::now();
    for Notification {
        created_at,
        content,
    } in state
        .notifications
        .iter()
        .filter(|n| (now - n.created_at).num_seconds() < state.config.notification_time as i64)
        .rev()
    {
        if cum_height > grid_area.height - state.config.notification_max_size.1 - 2 {
            break;
        }

        let (title, content, border_style, style) = match content {
            NotificationContent::Info(string) => (
                "Info",
                string.as_str(),
                Style::reset().fg(Color::Green),
                Style::reset(),
            ),
            NotificationContent::RuneError(err) => {
                ("Rune Error", err.as_str(), Style::reset(), Style::reset())
            }
            NotificationContent::Error(err) => (
                "Error",
                (**err).as_ref(),
                Style::reset().fg(Color::Red),
                Style::reset().fg(Color::Red),
            ),
        };

        // Attempt to apply ANSI color escape codes, otherwise default to raw print
        let text = content.into_text().unwrap_or(Text::from(content));

        let text_width = text
            .iter()
            .take(max_height as usize)
            .map(Line::width)
            .max()
            .unwrap_or(max_width as usize)
            // TODO: hard wrap lines like for tooltips
            // .min(max_width as usize) as u16;
            .min(grid_area.width as usize) as u16;

        let total_height = text.height().min(max_height as usize) as u16 + 2;
        cum_height += total_height;

        let text_margin = Margin::new(2, 1);

        let area = Rect {
            x: grid_area.right() - text_width - text_margin.horizontal * 2, // offset for the borders
            y: grid_area.bottom() - cum_height,
            width: text_width + text_margin.horizontal * 2,
            height: total_height,
        };

        // Frame
        frame.render_widget(
            Block::new()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style)
                .border_type(BorderType::Plain),
            area,
        );

        // Content
        frame.render_widget(
            Paragraph::new(text).style(style),
            area.inner(&Margin::new(2, 1)),
        );

        // Progress bar
        let progress_bar_width = area.width as usize - 2 - title.len();
        let progress = (progress_bar_width as f32 * (now - created_at).num_seconds() as f32
            / state.config.notification_time as f32) as usize;
        frame.buffer_mut().set_string(
            area.left() + title.len() as u16 + 1,
            area.top(),
            "━".repeat(progress_bar_width - progress),
            Style::default(),
        );
    }
}

fn render_tooltip(frame: &mut Frame, grid_area: Rect, state: &State) {
    if let Some(tooltip) = state.tooltip.clone() {
        let (title, content, style) = match tooltip {
            Tooltip::Input(mode, input) => (
                format!("Input ({:?})", mode),
                input,
                Style::default().fg(Color::Magenta),
            ),
            Tooltip::Command(cmd) => (
                "Command".to_owned(),
                cmd,
                Style::default().fg(Color::Yellow),
            ),
            Tooltip::Info(info) => ("Info".to_owned(), info, Style::default().fg(Color::Green)),
            Tooltip::Error(err) => ("Error".to_owned(), err, Style::default().fg(Color::Red)),
        };

        let lines = content
            .lines()
            .map(str::trim)
            .flat_map(|s| {
                s.chars()
                    .chunks(
                        grid_area.width as usize
                            - state.config.notification_max_size.0 as usize
                            - 2,
                    )
                    .into_iter()
                    .map(|chunk| chunk.collect::<String>())
                    .collect::<Vec<String>>()
            })
            .collect::<Vec<String>>();

        let command_area = Rect {
            x: grid_area.left(),
            y: grid_area.bottom() - 2 - lines.len().max(1) as u16,
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
            command_area.inner(&Margin::new(2, 1)),
        );
    }
}
