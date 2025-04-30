use crossterm::event::{self};
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use rayon::iter::*;
use tokio::io::{AsyncBufReadExt, BufReader, Stdin};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crossterm::execute;
use ratatui::Terminal;
use ratatui::style::Color;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use std::io::{self, Stderr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch::{Receiver, Sender};

use crate::{helpers, types};

pub fn stdin_reader(
    reader: BufReader<Stdin>,
    out_chan: UnboundedSender<Vec<String>>,
    batch_size: usize,
) {
    let mut lines = reader.lines();
    tokio::spawn(async move {
        let mut buff = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            buff.push(line);

            if buff.len() >= batch_size {
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
            (filtered_lines, ui_stuff, movement, lines) = tokio::select! {
                 _ = new_data_chan.changed() => {
                     let (list_size, new_l) = new_data_chan.borrow().clone();
                     if let Some(sel) = selected {
                         if sel >= new_l.len() {
                             selected = Some(new_l.len().saturating_sub(1));
                         }
                     }
                     (new_l, ui_stuff, None, list_size)
                },
                _ = ui_chan.changed() =>{
                    let ui_new = ui_chan.borrow().clone();
                    (filtered_lines, Some(ui_new), None, lines)
                },
                m = movement_chan.recv() => {
                    (filtered_lines, ui_stuff, m, lines)
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
                        input: String::new(),
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
                    let visible_len = filtered_lines.len().min(list_height);
                    let index_from_bottom =
                        selected.unwrap_or(0).min(visible_len.saturating_sub(1));
                    let index_from_top = visible_len
                        .saturating_sub(1)
                        .saturating_sub(index_from_bottom);
                    real_selected = Some(padding_rows + index_from_top);

                    let label = format!("[ {}/{} ]", selected.unwrap_or(0) + 1, lines);
                    let label_width = label.len() as u16;
                    let divider_fill = if chunks[1].width > label_width {
                        "─".repeat((chunks[1].width - label_width - 1) as usize)
                    } else {
                        String::new()
                    };

                    let divider_line = Paragraph::new(Line::from(vec![
                        Span::styled(label, Style::default().fg(Color::LightGreen)),
                        Span::raw(" "),
                        Span::styled(divider_fill, Style::default().fg(Color::LightCyan)),
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
                                    .skip(filtered_lines.len().saturating_sub(list_height))
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
            tokio::time::sleep(Duration::ZERO).await;
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
    mut source_chan: UnboundedReceiver<Vec<String>>,
    send_source_chan: UnboundedSender<Vec<String>>,
    buff_size: usize,
    score_clamp: usize,
    batch_size: usize,
) {
    let mut input = String::new();
    tokio::spawn(async move {
        let mut index: Vec<Option<Vec<(String, Vec<usize>)>>> = Vec::new();
        let mut count = 0; 
        loop {
            let query = tokio::select! {
                _ = in_chan.changed() => {
                    let r = in_chan.borrow().clone();
                    let ni = match r {
                        Some(r) => r.clone(),
                        None => input
                    };

                    let mut buff = Vec::new();

                    for  val in index {
                        if let Some(v) = val {
                            for (i,_) in v {
                                buff.push(i);
                                if buff.len() >= batch_size {
                                    let _ = send_source_chan.send(buff);
                                    buff = Vec::new();
                                }
                            }
                        }
                    }

                    let _ = send_source_chan.send(buff);
                    index = Vec::new();
                    ni
                },
                new_lines = source_chan.recv() => {
                    if let Some(x) = new_lines {
                        for i in x {
                            helpers::index_items(&mut index, i, &input, score_clamp);
                        }
                    }
                    input
                }
            };

            input = query.clone();
            let mut buff = Vec::new();

            let new_size = index.iter().fold(0, |a, b| {
                let mut ns = 0;
                if let Some(bb) = b {
                    ns += bb.len();
                }
                ns + a
            });
            count = count.max(new_size);
            for i in &index {
                if let Some(j) = i {
                    let slice = j[..buff_size.min(j.len())].to_vec();
                    buff.extend(slice);
                }
                if buff.len() > buff_size {
                    break;
                }
            }

            //buff.reverse();

            let _ = out_chan.send((count, buff));
        }
    });
}
