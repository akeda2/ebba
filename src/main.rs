mod app;
mod cli;
mod command;
mod config;
mod document;
mod input;
mod terminal;
mod ui;

fn main() {
    if let Err(error) = app::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
