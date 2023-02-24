mod frontend;
mod grid;
mod logic;

use std::{sync::mpsc, thread::JoinHandle};

use anyhow::bail;
use clap::Parser;

use anyhow::Result;

#[derive(Parser)]
/// Minesweeper TUI editor and runner
struct Args {
    /// Input file location
    input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (sender, receiver) = mpsc::channel();

    let handler = std::thread::spawn(move || logic::run(args, sender));

    if let Err(err) = frontend::run(receiver) {
        join_handler(handler)?;
        std::fs::write("log", format!("{err:?}"))?;
        bail!("{err:?}");
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
