use std::collections::VecDeque;

use crate::grid::Grid;

use {arboard::Clipboard, tui::style::Color};

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

    pub command_history: VecDeque<String>,
    pub command_history_index: Option<usize>,

    pub clipboard: Clipboard,

    pub debug: Option<String>,
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
}

impl From<&EditorMode> for Color {
    fn from(value: &EditorMode) -> Self {
        match value {
            EditorMode::Normal => Color::White,
            EditorMode::Command(_) => Color::DarkGray,
            EditorMode::Visual(_, _) => Color::Cyan,
            EditorMode::Insert => Color::Yellow,
            EditorMode::Running => Color::Red,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Tooltip {
    Command(String),
    Info(String),
    Error(String),
}
