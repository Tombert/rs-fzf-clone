use clap::Parser;
use crossterm::event::EnableMouseCapture;
use crossterm::terminal::enable_raw_mode;
use ratatui::backend::CrosstermBackend;
use tokio::io::BufReader;

use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use ratatui::Terminal;
use ratatui::widgets::ListState;
mod helpers;
mod processors;
mod types;

use std::io::{self};

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = types::Args::parse();

    let buffsize = args.buffsize.unwrap_or(100);
    let batchsize = args.batchsize.unwrap_or(50);
    let scoreclamp = args.scoreclamp.unwrap_or(50);

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let (ui_send, ui_recv) = tokio::sync::watch::channel::<types::UIStuff>(types::UIStuff {
        cursor_position: 0,
        input: String::new(),
        enter: false,
    });
    let (input_send, input_recv) = tokio::sync::watch::channel::<Option<String>>(None);
    let (processed_send, processed_recv) =
        tokio::sync::watch::channel::<(usize, Vec<(String, Vec<usize>)>)>((0, Vec::new()));
    let (movement_send, movement_recv) = tokio::sync::mpsc::unbounded_channel::<types::Movement>();
    let (all_line_send, all_lines_recv) =
        tokio::sync::mpsc::unbounded_channel::<Vec<String>>();

    let list_state = ListState::default();

    enable_raw_mode()?;
    let mut screen = io::stderr();
    execute!(screen, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(screen);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    let _ = input_send.send(None);
    processors::handle_input(ui_send, input_send.clone(), movement_send);
    processors::process_input(
        input_recv,
        processed_send.clone(),
        all_lines_recv,
        all_line_send.clone(),
        buffsize,
        scoreclamp,
    );
    processors::stdin_reader(reader, all_line_send.clone(), batchsize);

    processors::render(terminal, list_state, processed_recv, ui_recv, movement_recv);
    futures::future::pending::<()>().await;
    Ok(())
}
