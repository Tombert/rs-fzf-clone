use crossterm::event::{self};
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use itertools::Itertools;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use tokio::io::{AsyncBufReadExt, BufReader, Stdin};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crossterm::execute;
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

pub fn stdin_reader2(
    reader: BufReader<Stdin>,
    out_chan: UnboundedSender<Vec<(String, Vec<usize>)>>,
) {
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

pub fn render(
    mut terminal: Terminal<CrosstermBackend<Stderr>>,
    mut list_state: ListState,
    mut new_data_chan: Receiver<(usize, Vec<(String, Vec<usize>)>)>,
    mut ui_chan: Receiver<types::UIStuff>,
    mut movement_chan: UnboundedReceiver<types::Movement>,
) {
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        let mut filtered_lines: Vec<(String, Vec<usize>)> = Vec::new();
        let mut ui_stuff = None;
        let mut selected = None;
        let mut real_selected: Option<usize> = None;
        let def = if filtered_lines.len() > 0 {
            filtered_lines.len() - 1
        } else {
            0
        };
        let mut lines = 0;
        loop {
            let t = selected.unwrap_or(def);
            let movement;
            selected = Some(t);
            (filtered_lines, ui_stuff, movement) = tokio::select! {
                 _ = new_data_chan.changed() => {
                     let (list_size, new_l) = new_data_chan.borrow().clone();
                     lines = list_size;
                    (new_l, ui_stuff, None)
                },
                _ = ui_chan.changed() =>{
                    let ui_new = ui_chan.borrow().clone();
                    (filtered_lines, Some(ui_new), None)
                },
                m = movement_chan.recv() => {
                    (filtered_lines, ui_stuff, m)
                }
            };

            terminal
                .draw(|f| {
                    let size = f.size();
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints(
                            [
                                Constraint::Min(1),
                                Constraint::Length(1),
                                Constraint::Length(3),
                            ]
                            .as_ref(),
                        )
                        .split(size);

                    let ui = ui_stuff.clone().unwrap_or(types::UIStuff {
                        cursor_position: 0,
                        input: "".to_string(),
                        enter: false,
                    });

                    let list_height = chunks[0].height as usize;
                    let actual_items_to_show = filtered_lines.len().min(list_height);

                    let padding_rows = list_height.saturating_sub(actual_items_to_show);

                    let start_idx = if filtered_lines.len() > list_height {
                        filtered_lines.len() - list_height
                    } else {
                        0
                    };
                    if let Some(m) = movement {
                        match m {
                            types::Movement::Down => {
                                let current_selected = selected.unwrap_or(0);
                                if current_selected > 0 {
                                    let new_selected = current_selected - 1;
                                    selected = Some(new_selected);
                                }
                            }
                            types::Movement::Up => {
                                let current_selected = selected.unwrap_or(0);
                                let new_selected = current_selected + 1;
                                selected = Some(new_selected);
                            }

                            types::Movement::Enter => {
                                if let Some(sel) = real_selected {
                                    let selected_idx = sel.saturating_sub(padding_rows) + start_idx;
                                    if let Some(line) = filtered_lines.get(selected_idx) {
                                        let _ = disable_raw_mode();
                                        let _ = execute!(io::stderr(), LeaveAlternateScreen);
                                        println!("{}", line.0);
                                        std::process::exit(0);
                                    }
                                }
                            }
                        }
                    }
                    let index_from_bottom = selected.unwrap_or(0);
                    let max_idx = filtered_lines.len().saturating_sub(1);
                    let index_from_top = max_idx.saturating_sub(index_from_bottom);
                    real_selected = Some(padding_rows + index_from_top.saturating_sub(start_idx));

                    let label = format!("[ {}/{} ]", selected.unwrap_or(0), lines);
                    let label_width = label.len() as u16;
                    let divider_fill = if chunks[1].width > label_width {
                        "─".repeat((chunks[1].width - label_width - 1) as usize)
                    } else {
                        "".to_string()
                    };

                    let divider_line = Paragraph::new(Line::from(vec![
                        Span::styled(label, Style::default().fg(Color::Gray)),
                        Span::raw(" "),
                        Span::styled(divider_fill, Style::default().fg(Color::DarkGray)),
                    ]));
                    f.render_widget(divider_line, chunks[1]);

                    let input_para = Paragraph::new(Text::from(vec![Line::from(vec![
                        Span::styled("> ", Style::default().fg(Color::Blue)),
                        Span::raw(ui.clone().input),
                    ])]))
                    .block(Block::default().borders(Borders::NONE));
                    f.render_widget(input_para, chunks[2]);
                    f.set_cursor(chunks[2].x + 2 + ui.cursor_position as u16, chunks[2].y);

                    let items_to_render = {
                        let items = (0..padding_rows)
                            .map(|_| ListItem::new(""))
                            .chain(
                                filtered_lines
                                    .iter()
                                    .take(list_height)
                                    .map(|(line, hits)| helpers::styled_line(line, hits)),
                            )
                            .collect::<Vec<_>>();
                        items
                    };

                    let list = List::new(items_to_render)
                        .block(Block::default().borders(Borders::NONE))
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                    list_state.select(real_selected);

                    f.render_stateful_widget(list, chunks[0], &mut list_state);
                })
                .unwrap();
            tokio::task::yield_now().await;
        }
    });
}

pub fn handle_input(
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
                    if let Some(mut x) = new_lines {
                        all_lines.append(&mut x);  // Moves out of `x`, no clone!
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
