use super::InputMode;

use chrono::{DateTime, Local};
use rune::Any;

pub struct Notification {
    pub created_at: DateTime<Local>,
    pub content: NotificationContent,
}

pub enum NotificationContent {
    Info(String),
    CommandResult(String),
    RuneError(String),
    Error(Box<dyn AsRef<str>>),
}

#[derive(Clone, Debug, Any)]
#[allow(unused)]
pub enum Tooltip {
    #[rune(constructor)]
    Input(#[rune(get)] InputMode, #[rune(get)] String),
    #[rune(constructor)]
    Command(#[rune(get)] String),
    #[rune(constructor)]
    Info(#[rune(get)] String),
    #[rune(constructor)]
    Error(#[rune(get)] String),
}

impl Notification {
    pub fn new(content: NotificationContent) -> Self {
        Self {
            created_at: Local::now(),
            content,
        }
    }
}
