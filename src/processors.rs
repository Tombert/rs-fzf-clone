use crossterm::event::{self, EnableMouseCapture};
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use itertools::Itertools;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use tokio::io::{AsyncBufReadExt, BufReader, Stdin};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use ratatui::Terminal;
use ratatui::style::Color;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use std::collections::HashMap;
use std::io::{self, Stderr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch::{Receiver, Sender};

use crate::{helpers, types};

pub fn stdin_reader2(reader: BufReader

    <Stdin>, out_chan: UnboundedSender<Vec<(String, Vec<usize>)>>) {
    let mut lines = reader.lines();
    tokio::spawn(async move {
        let mut buff = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            buff.push((line, Vec::new()));

            if buff.len() >= 2000 {
                let _ = out_chan.send(buff);
                buff = Vec::new(); 
            }
        }
        let _ = out_chan.send(buff.clone());
    });
}



fn handle_input(
    ui_out_chan: Sender<types::UIStuff>,
    process_chan: Sender<Option<String>>,
    movement_chan: UnboundedSender<types::Movement>,
) {
    tokio::spawn(async move {
        let mut last_ui = types::UIStuff {
            input: String::new(),
            enter: false,
            cursor_position: 0,
        };

        let mut current_ui = last_ui.clone();

        let mut start = SystemTime::now().duration_since(UNIX_EPOCH).expect("");

        loop {
            tokio::task::yield_now().await;
            if let Ok(_) = event::poll(Duration::from_millis(50)) {
                let res = match event::read() {
                    Ok(e) => helpers::parse_action(e),
                    _ => types::Action::Other,
                };
                match res {
                    types::Action::Key(c) => {
                        if current_ui.cursor_position <= current_ui.input.len() {
                            current_ui.input.insert(current_ui.cursor_position, c);
                            current_ui.cursor_position += 1;
                        }
                    }
                    types::Action::BackSpace => {
                        if current_ui.cursor_position > 0 {
                            current_ui.input.remove(current_ui.cursor_position - 1);
                            current_ui.cursor_position -= 1;
                        }
                    }
                    types::Action::ClearAll => {
                        current_ui.cursor_position = 0;
                        current_ui.input.clear();
                    }
                    types::Action::Select => {
                        let _ = movement_chan.send(types::Movement::Enter);
                    }
                    types::Action::Exit => {
                        let _ = disable_raw_mode();
                        let _ = execute!(io::stderr(), LeaveAlternateScreen);
                        std::process::exit(0);
                    }
                    types::Action::MoveBegin => {
                        current_ui.cursor_position = 0;
                    }
                    types::Action::MoveEnd => {
                        current_ui.cursor_position = current_ui.input.len();
                    }
                    types::Action::MoveLeft => {
                        if current_ui.cursor_position > 0 {
                            current_ui.cursor_position -= 1;
                        }
                    }
                    types::Action::MoveRight => {
                        if current_ui.cursor_position < current_ui.input.len() {
                            current_ui.cursor_position += 1;
                        }
                    }
                    types::Action::MoveUp => {
                        let _ = movement_chan.send(types::Movement::Up);
                    }
                    types::Action::MoveDown => {
                        let _ = movement_chan.send(types::Movement::Down);
                    }
                    types::Action::Other => (),
                }
            }

            if current_ui != last_ui {
                let end = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time went backwards");
                if end.saturating_sub(start) > Duration::from_millis(100)
                    && current_ui.input != last_ui.input
                {
                    let _ = process_chan.send(Some(current_ui.input.clone()));
                    start = SystemTime::now().duration_since(UNIX_EPOCH).expect("");
                }
                let _ = ui_out_chan.send(current_ui.clone());
                last_ui = current_ui.clone();
            }
        }
    });
}



pub fn process_input(
    mut in_chan: Receiver<Option<String>>,
    out_chan: Sender<(usize, Vec<(String, Vec<usize>)>)>,
    mut source_chan: UnboundedReceiver<Vec<(String, Vec<usize>)>>, 
) {
    //let all_lines = all_lines.clone();
    let mut input = "".to_string();
    const BUFF_SIZE: usize = 100;
    tokio::spawn(async move {
        let mut all_lines = Vec::new();
        loop {
            let query = tokio::select! {
                _ = in_chan.changed() => {
                    let r = in_chan.borrow().clone();
                    match r {
                        Some(r) => r.clone(),
                        None => input
                    }
                },
                new_lines = source_chan.recv() => {
                    if let Some(x) = new_lines {
                        all_lines.extend(x);
                    }
                    input
                }
            };

            input = query.clone();
            let input2 = input.clone();

            if !query.is_empty() {
                let (new_all_lines, buff) = tokio::task::spawn_blocking(move || {
                    let indexed = all_lines
                        .iter()
                        .filter_map(|(line, _)| {
                            helpers::fuzzy_search(input2.as_str(), line.as_str())
                        })
                        .fold(HashMap::new(), |mut acc, (s, v)| {
                            let key = helpers::get_delta(&v);
                            acc.entry(key).or_insert_with(Vec::new).push((s, v));
                            acc
                        });

                    let mut buff = Vec::new();
                    for key in indexed.keys().sorted().cloned() {
                        let temp = Vec::new();
                        let current = indexed.get(&key).unwrap_or(&temp);
                        let slice = current[..BUFF_SIZE.min(current.len())].to_vec();
                        buff.extend(slice);
                        if buff.len() >= BUFF_SIZE {
                            break;
                        }
                    }
                    buff.reverse();
                    (all_lines, buff)
                })
                .await
                .expect("");
                all_lines = new_all_lines;
                let _ = out_chan.send((all_lines.len(), buff));
            } else {
                let al = all_lines[..BUFF_SIZE.min(all_lines.len())].to_vec();
                let _ = out_chan.send((all_lines.len(), al));
            }
        }
    });
}
