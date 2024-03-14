mod cell;
mod frontend;
mod grid;
mod logic;

use std::{sync::mpsc, thread::JoinHandle};

use {
    anyhow::{bail, Result},
    clap::Parser,
    crossterm::terminal::disable_raw_mode,
};

#[derive(Parser)]
/// Minesweeper TUI editor and runner
struct Args {
    /// Input file location
    input: String,
}

fn main() -> Result<()> {
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        disable_raw_mode().unwrap();
        default_panic_hook(info);
    }));

    let args = Args::parse();

    let (frontend_sender, frontend_receiver) = mpsc::channel();
    let (logic_sender, logic_receiver) = mpsc::channel();

    let handler = std::thread::spawn(move || logic::run(args, frontend_sender, logic_receiver));

    if let Err(err) = frontend::run(frontend_receiver, logic_sender) {
        join_handler(handler)?;
        bail!("{err}");
    }

    join_handler(handler)?;

    Ok(())
}

fn join_handler<T>(handler: JoinHandle<T>) -> Result<()> {
    if let Err(err) = handler.join() {
        if let Some(err) = err.downcast_ref::<logic::Error>() {
            return Err(err.clone().into());
        } else {
            bail!("Unhandled logic error type");
        }
    }

    Ok(())
}
