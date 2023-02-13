mod frontend;
mod logic;

use std::{sync::mpsc, thread::JoinHandle};

use anyhow::bail;

use {anyhow::Result, pancurses::endwin};

fn main() -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    // frontend::run(receiver):

    let handler = std::thread::spawn(move || logic::run(sender));

    if let Err(err) = frontend::run(receiver) {
        endwin();
        join_handler(handler)?;
        bail!("{err:?}");
    }

    endwin();

    Ok(())
}

fn join_handler<T>(handler: JoinHandle<T>) -> Result<()> {
    if let Err(err) = handler.join() {
        if let Some(err) = err.downcast_ref::<frontend::Error>() {
            return Err(err.clone().into());
        } else {
            bail!("Unhandled frontend error type");
        }
    }

    Ok(())
}
