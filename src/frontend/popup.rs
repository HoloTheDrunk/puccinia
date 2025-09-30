use std::fmt::Debug;

use super::InputMode;

use {
    anyhow::anyhow,
    chrono::{DateTime, Local},
    rune::Any,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub count: usize,
    pub created_at: DateTime<Local>,
    pub content: NotificationContent,
}

#[derive(Debug)]
pub enum NotificationContent {
    Info(String),
    CommandResult(String),
    RuneError(String),
    Error(anyhow::Error),
}

impl Clone for NotificationContent {
    fn clone(&self) -> Self {
        match self {
            Self::Info(s) => Self::Info(s.clone()),
            Self::CommandResult(s) => Self::CommandResult(s.clone()),
            Self::RuneError(s) => Self::RuneError(s.clone()),
            Self::Error(e) => Self::Error(anyhow!(e.to_string())),
        }
    }
}

impl PartialEq for NotificationContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Info(a), Self::Info(b)) => a == b,
            (Self::CommandResult(a), Self::CommandResult(b)) => a == b,
            (Self::RuneError(a), Self::RuneError(b)) => a == b,
            (Self::Error(a), Self::Error(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
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
            count: 1,
            created_at: Local::now(),
            content,
        }
    }
}
