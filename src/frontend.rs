use std::sync::mpsc::{self, Receiver};

use pancurses::chtype;

use {
    ellipse::Ellipse,
    pancurses::{endwin, initscr, noecho, Input, Window},
};

#[doc(hidden)]
const GREY_PAIR: chtype = 1;
const GREEN_PAIR: chtype = 2;
const RED_PAIR: chtype = 3;

#[derive(thiserror::Error, Clone, Debug)]
pub enum Error {
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("Channel error: {0:?}")]
    ChannelError(mpsc::TryRecvError),
}

type Result<T> = anyhow::Result<T, Error>;

#[derive(Debug)]
#[allow(unused)]
pub enum Message {
    Load(String),
    Break,
    LogicFail(Option<String>),
}

pub fn run(receiver: Receiver<Message>) -> Result<()> {
    let window = setup();

    main_loop(&window, &receiver)?;

    wait_for_exit(&window);
    endwin();

    Err(Error::Unknown("Oopsie".to_owned()))
}

fn setup() -> Window {
    let window = initscr();

    window.keypad(true);
    window.draw_box(0 as char, 0 as char);
    window.refresh();

    noecho();

    // Color setup
    let mut bg = pancurses::COLOR_BLACK;

    pancurses::start_color();
    if pancurses::has_colors() {
        if pancurses::use_default_colors() == pancurses::OK {
            bg = -1;
        }

        pancurses::init_pair(GREY_PAIR as i16, pancurses::COLOR_WHITE, bg);
        pancurses::init_pair(GREEN_PAIR as i16, pancurses::COLOR_GREEN, bg);
        pancurses::init_pair(RED_PAIR as i16, pancurses::COLOR_RED, bg);
    }

    window
}

fn main_loop(window: &Window, receiver: &Receiver<Message>) -> Result<()> {
    loop {
        match receiver.try_recv() {
            Ok(Message::Load(content)) => {
                window.mv(1, 1);
                window.printw(content);
            }
            Ok(Message::Break) => break,
            Ok(Message::LogicFail(opt_msg)) => {
                if let Some(msg) = opt_msg {
                    print_error(&window, msg);
                }
            }
            Err(err) => {
                return Err(Error::ChannelError(err));
            }
        }

        if let Some(c) = window.getch() {
            match c {
                Input::Character(c) => window.addch(c),
                _ => break,
            };
        }
    }

    Ok(())
}

fn print_error(window: &Window, msg: impl ToString) {
    set_color(&window, RED_PAIR, true, true);
    window.mv(window.get_max_y() - 1, 0);
    window.printw(
        format!("{}", msg.to_string())
            .as_str()
            .truncate_ellipse((window.get_max_x() - 3) as usize),
    );
}

fn wait_for_exit(window: &Window) {
    while let Some(c) = window.getch() {
        match c {
            Input::KeyDC => break,
            _ => (),
        }
    }
}

/// Sets or unsets a color and optionally a boldness.
///
/// # Example
/// ```
/// let win = init_frontend();
///
/// set_color(&win, GREEN_PAIR, true, true);
///
/// win.addch('c'); // Should print a bold green 'c'
/// ```
fn set_color(window: &pancurses::Window, pair: chtype, bold: bool, enabled: bool) {
    if pancurses::has_colors() {
        let mut attr = pancurses::COLOR_PAIR(pair);

        if bold {
            attr |= pancurses::A_BOLD;
        }

        if enabled {
            window.attrset(attr);
        } else {
            window.attroff(attr);
        }
    }
}
