mod frontend;
mod logic;

use std::sync::mpsc;

use anyhow::bail;

use {anyhow::Result, pancurses::endwin};

fn main() -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    // frontend::run(receiver):

    std::thread::spawn(move || logic::run(sender));

    if let Err(err) = frontend::run(receiver) {
        endwin();
        bail!("{err:?}");
    }

    // if let Err(err) = handler.join() {
    //     if let Some(err) = err.downcast_ref::<frontend::Error>() {
    //         return Err(err.clone().into());
    //     } else {
    //         bail!("Unhandled frontend error type");
    //     }
    // }

    endwin();

    Ok(())
}
