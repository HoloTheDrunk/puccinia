use super::{
    popup::{Notification, Tooltip},
    Keybindings,
};

use crate::grid::Grid;

use std::{collections::VecDeque, sync::Arc};

use {arboard::Clipboard, ratatui::style::Color, rune::Any};

#[derive(Clone, Default, Debug, Any)]
pub struct Config {
    // Side area for run information
    pub run_area_width: u16,
    pub run_area_position: RunAreaPosition,
    pub output_area_height: u16,

    // Editor display settings
    #[rune(get, set)]
    pub heat: bool,
    pub lids: bool,
    pub sides: bool,

    // Notifications list settings
    pub notification_max_size: (u16, u16),
    pub notification_capacity: u32,
    pub notification_time: u32,

    // Running mode optimizations
    #[rune(get, set)]
    pub live_output: bool,

    // Keybindings
    pub bindings: Keybindings,

    // Autosave
    pub cache_directory: String,
    pub cache_directory_size: u64,
    pub autosave_interval: u64,
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

pub struct RuneState {
    runtime: Arc<rune::runtime::RuntimeContext>,
    unit: Arc<rune::Unit>,
}

impl RuneState {
    pub fn new(runtime: rune::runtime::RuntimeContext, unit: rune::Unit) -> Self {
        Self {
            runtime: Arc::new(runtime),
            unit: Arc::new(unit),
        }
    }

    pub fn to_vm(&self) -> rune::Vm {
        rune::Vm::new(self.runtime.clone(), self.unit.clone())
    }
}

#[derive(Any)]
pub struct State {
    #[rune(get, set)]
    pub mode: EditorMode,
    pub previous_mode: Option<EditorMode>,

    #[rune(get)]
    pub grid: Grid,
    pub stack: Vec<i32>,
    pub output: String,
    pub output_buffer: Option<String>,

    #[rune(get)]
    pub tooltip: Option<Tooltip>,
    pub notifications: Vec<Notification>,
    #[rune(get)]
    pub config: Config,

    #[rune(get)]
    pub history: GridHistory,

    pub command_history: VecDeque<String>,
    pub command_history_index: Option<usize>,

    pub clipboard: Clipboard,

    pub debug: Option<String>,

    pub rune_state: Option<RuneState>,
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

#[derive(Clone, Debug, Any)]
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

#[derive(Clone, Default, Debug, PartialEq, Eq, Any)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Any)]
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
