use std::{collections::VecDeque, str::Lines};

use crate::grid::Grid;

use {arboard::Clipboard, itertools::Itertools, tui::style::Color};

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

pub struct State {
    pub mode: EditorMode,
    pub previous_mode: Option<EditorMode>,

    pub grid: Grid,
    pub stack: Vec<i32>,
    pub output: String,
    pub output_buffer: Option<String>,

    pub tooltip: Option<Tooltip>,
    pub config: Config,

    pub history: GridHistory,

    pub command_history: VecDeque<String>,
    pub command_history_index: Option<usize>,

    pub clipboard: Clipboard,

    pub debug: Option<String>,
}

impl State {
    pub fn push_history(&mut self) {
        let mut cgrid = self.grid.clone();
        cgrid.trim();

        let dump = cgrid.dump();

        // Avoid pushing the same effective state twice
        if dump == self.history.inner.back().cloned().unwrap_or_default() {
            return;
        }

        if self.history.inner.len() + 1 > self.history.max_size {
            self.history.inner.pop_front();
        }

        self.history.inner.push_back(dump);
    }

    pub fn load_history(&mut self, index: usize) -> bool {
        self.history
            .inner
            .get((self.history.inner.len() - index).saturating_sub(1))
            .map(|string| self.grid.load_values(string.clone()))
            .is_some()
    }
}

pub struct GridHistory {
    pub inner: VecDeque<String>,
    pub max_size: usize,
}

impl GridHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum EditorMode {
    #[default]
    /// Mode for moving around efficiently and running commands
    Normal,
    /// Command input mode
    Command(String),
    /// Text selection mode
    Visual((usize, usize), (usize, usize)),
    /// Text insertion mode
    Insert,
    /// Running state
    Running,
    /// Interactive input mode (& and ~)
    Input(InputMode, String),
    /// Grid history browsing mode
    History(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Integer,
    ASCII,
}

impl From<&EditorMode> for Color {
    fn from(value: &EditorMode) -> Self {
        match value {
            EditorMode::Normal => Color::White,
            EditorMode::Command(_) | EditorMode::Input(_, _) => Color::DarkGray,
            EditorMode::Visual(_, _) => Color::Cyan,
            EditorMode::Insert => Color::Yellow,
            EditorMode::Running => Color::Red,
            EditorMode::History(_) => Color::LightMagenta,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Tooltip {
    Input(InputMode, String),
    Command(String),
    Info(String),
    Error(String),
}
