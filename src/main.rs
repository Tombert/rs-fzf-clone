use crossterm::event::{self, EnableMouseCapture};
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
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
mod helpers;

use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{self, Stderr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

fn styled_line(line: &str, hits: &Vec<usize>) -> ListItem<'static> {
    let mut spans = Vec::with_capacity(line.len());
    for (i, c) in line.chars().enumerate() {
        if hits.contains(&i) {
            spans.push(Span::styled(
                c.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(c.to_string(), Style::default()));
        }
    }
    ListItem::new(Text::from(vec![Line::from(spans)]))
}

fn stdin_reader(
    state: Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    reader: BufReader<Stdin>,
    out_chan: UnboundedSender<Option<String>>,
) {
    let mut lines = reader.lines();
    tokio::spawn(async move {
        let mut counter = 0;
        while let Ok(Some(line)) = lines.next_line().await {
            state.write().await.push((line, Vec::new()));
            if counter == 0 {
                let _ = out_chan.send(None);
            }
            counter = (counter + 1) % 10000;
        }
    });
}

fn render(
    all_lines: &Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    mut terminal: Terminal<CrosstermBackend<Stderr>>,
    mut list_state: ListState,
    mut new_data_chan: UnboundedReceiver<Vec<(String, Vec<usize>)>>,
    mut ui_chan: UnboundedReceiver<UIStuff>,
    mut movement_chan : UnboundedReceiver<Movement>
) {
    let z = all_lines.clone();
    tokio::spawn(async move {
        let mut filtered_lines: Vec<(String, Vec<usize>)> = Vec::new();
        let mut ui_stuff = None;
        let mut selected = None; 
        let def = if filtered_lines.len() > 0 {filtered_lines.len() - 1} else {0};
        loop {
            let t = selected.unwrap_or(def);
            let mut hit_enter = false; 
            let mut movement = None; 
            selected = Some(t);
            (filtered_lines, ui_stuff, movement) = tokio::select! {
                new_l = new_data_chan.recv() => {
                    (new_l.unwrap_or(Vec::new()), ui_stuff, None)
                },
                ui_new = ui_chan.recv() =>{
                    (filtered_lines, ui_new, None)
                },
                m = movement_chan.recv() => {
                    (filtered_lines, ui_stuff, m)
                }
            };


            let total_len = z.clone().read().await.len();
            tokio::task::block_in_place(|| {
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

                        let ui = ui_stuff.clone().unwrap_or(UIStuff {
                            cursor_position: 0,
                            input: "".to_string(),
                            enter: false,
                        });
                        if let Some(m) = movement {
                            match m {
                                Movement::Up => {
                                    let current_selected = selected.unwrap_or(0);
                                    if current_selected > 0 {
                                        let new_selected = current_selected - 1; 
                                        selected = Some(new_selected); 
                                    }
                                }, 
                                Movement::Down => {
                                    let current_selected = selected.unwrap_or(0);
                                    let new_selected = current_selected + 1; 
                                    selected = Some(new_selected); 
                                },

                                Movement::Enter => {
                                    if let Some(sel) = selected {
                                        if let Some(line) = filtered_lines.get(sel) {
                                            let _ = disable_raw_mode();
                                            let _ = execute!(io::stderr(), LeaveAlternateScreen);
                                            println!("{}", line.0);
                                            std::process::exit(0);
                                        }

                                    }
                                }

                            }
                        }

                        //let selected_display = ui.selected.unwrap_or(0) + 1; // 1-based indexing
                        let label = format!("[ {}/{} ]", selected.unwrap_or(0), total_len);
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

                        let list_height = chunks[0].height as usize;
                        let actual_items_to_show = filtered_lines.len().min(list_height);

                        let padding_rows = list_height.saturating_sub(actual_items_to_show);

                        let (items_to_render, real_selected) =
                            if filtered_lines.len() <= list_height {
                                let padded_items = (0..padding_rows)
                                    .map(|_| ListItem::new(""))
                                    .chain(
                                        filtered_lines[..filtered_lines.len().min(100)]
                                        .iter()
                                        .map(|(line, hits)| styled_line(line, hits)),
                                    )
                                    .collect::<Vec<_>>();

                                let real_selected = Some(padded_items.len().saturating_sub(1));
                                (padded_items, real_selected)
                            } else {
                                let selected_idx = selected.unwrap_or(0);
                                let start_idx = if selected_idx + 1 >= list_height {
                                    selected_idx + 1 - list_height
                                } else {
                                    0
                                };
                                let items = filtered_lines
                                    .par_iter()
                                    .skip(start_idx)
                                    .take(list_height)
                                    .map(|(line, hits)| styled_line(line, hits))
                                    .collect::<Vec<_>>();

                                // 🔥 This is the fix:
                                let real_selected = Some(items.len().saturating_sub(1));
                                (items, real_selected)
                            };

                        let list = List::new(items_to_render)
                            .block(Block::default().borders(Borders::NONE))
                            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                        list_state.select(real_selected);


                        f.render_stateful_widget(list, chunks[0], &mut list_state);
                    })
                    .unwrap();
            });
        }
    });
}

#[derive(Clone, Eq, PartialEq)]
struct UIStuff {
    input: String,
    cursor_position: usize,
    enter: bool,
}

fn process_input(
    mut in_chan: UnboundedReceiver<Option<String>>,
    out_chan: UnboundedSender<Vec<(String, Vec<usize>)>>,
    all_lines: &Arc<RwLock<Vec<(String, Vec<usize>)>>>,
) {
    let all_lines = all_lines.clone();
    let mut input = "".to_string();
    const BUFF_SIZE : usize = 40;
    tokio::spawn(async move {
        loop {
            let query = match in_chan.recv().await {
                Some(Some(r)) => r.clone(),
                Some(None) => input,
                None => input,
            };

            input = query.clone();

            if !query.is_empty() {
                let indexed = all_lines
                    .read()
                    .await
                    .par_iter()
                    .filter_map(|(line, _)| helpers::fuzzy_search(query.as_str(), line.as_str()))
                    .fold(HashMap::new, |mut acc, (s, v)| {
                        let key = helpers::get_delta(&v);
                        acc.entry(key).or_insert_with(Vec::new).push((s, v));
                        acc
                    })
                    .reduce(HashMap::new, |mut map1, map2| {
                        for (key, mut vec) in map2 {
                            map1.entry(key).or_insert_with(Vec::new).append(&mut vec);
                        }
                        map1
                    });

                let mut buff = Vec::new();
                for i in 0..50 {
                    let temp = Vec::new();
                    let current = indexed.get(&(50 - i)).unwrap_or(&temp);
                    let slice = current[..BUFF_SIZE.min(current.len())].to_vec();
                    buff.extend(slice);
                    if buff.len() >= BUFF_SIZE {
                        break;
                    }
                }

                let _ = out_chan.send(buff);
                tokio::task::yield_now().await;
            } else {
                let all_lines = all_lines.read().await;
                let al = all_lines[..BUFF_SIZE.min(all_lines.len())].to_vec();
                let _ = out_chan.send(al);
                tokio::task::yield_now().await;
            }
        }
    });
}

enum Movement {
    Up,
    Down,
    Enter
}

fn handle_input(
    ui_out_chan: UnboundedSender<UIStuff>,
    process_chan: UnboundedSender<Option<String>>,
    movement_chan: UnboundedSender<Movement>
) {
    tokio::spawn(async move {
        let mut last_ui = UIStuff {
            input: String::new(),
            enter: false,
            cursor_position: 0,
        };

        let mut current_ui = last_ui.clone();

        let mut start = SystemTime::now().duration_since(UNIX_EPOCH).expect("");

        loop {
            if let Ok(_) = event::poll(Duration::from_millis(50)) {
                let res = match event::read() {
                    Ok(e) => helpers::parse_action(e),
                    _ => helpers::Action::Other,
                };
                match res {
                    helpers::Action::Key(c) => {
                        if current_ui.cursor_position <= current_ui.input.len() {
                            current_ui.input.insert(current_ui.cursor_position, c);
                            current_ui.cursor_position += 1;
                        }
                    }
                    helpers::Action::BackSpace => {
                        if current_ui.cursor_position > 0 {
                            current_ui.input.remove(current_ui.cursor_position - 1);
                            current_ui.cursor_position -= 1;
                        }
                    }
                    helpers::Action::ClearAll => {
                        current_ui.cursor_position = 0;
                        current_ui.input.clear();
                    }
                    helpers::Action::Select => {
                        movement_chan.send(Movement::Enter); 
                    },
                    helpers::Action::Exit => {
                        let _ = disable_raw_mode();
                        let _ = execute!(io::stderr(), LeaveAlternateScreen);
                        std::process::exit(0);
                    }
                    helpers::Action::MoveBegin => {
                        current_ui.cursor_position = 0;
                    }
                    helpers::Action::MoveEnd => {
                        current_ui.cursor_position = current_ui.input.len();
                    }
                    helpers::Action::MoveLeft => {
                        if current_ui.cursor_position > 0 {
                            current_ui.cursor_position -= 1;
                        }
                    }
                    helpers::Action::MoveRight => {
                        if current_ui.cursor_position < current_ui.input.len() {
                            current_ui.cursor_position += 1;
                        }
                    }
                    helpers::Action::MoveUp => {
                        
                        movement_chan.send(Movement::Up);

                        // if let Some(new_selected) = current_ui.selected.clone() {
                        //     let ns = new_selected.clone();
                        //     if ns > 0 {
                        //         current_ui.selected = Some(ns - 1);
                        //     }
                        // }
                    }
                    helpers::Action::MoveDown => {
                        movement_chan.send(Movement::Down);
                        // if let Some(new_selected) = current_ui.selected.clone() {
                        //     let ns = new_selected.clone();
                        //     current_ui.selected = None;
                        // }
                    }
                    helpers::Action::Other => (),
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
            tokio::task::yield_now().await;
        }
    });
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin(); // 
    let reader = BufReader::new(stdin);
    let all_lines = Arc::new(RwLock::new(Vec::new()));
    let filtered_lines = Arc::new(RwLock::new(Vec::new()));
    let (ui_send, ui_recv) = tokio::sync::mpsc::unbounded_channel::<UIStuff>();
    let (input_send, input_recv) = tokio::sync::mpsc::unbounded_channel::<Option<String>>();
    let (processed_send, processed_recv) =
        tokio::sync::mpsc::unbounded_channel::<Vec<(String, Vec<usize>)>>();
    let (movement_send, movement_recv) =
        tokio::sync::mpsc::unbounded_channel::<Movement>();
    //let (source_send, source_recv) = tokio::sync::watch::channel::<Option<usize>>(None);

    stdin_reader(all_lines.clone(), reader, input_send.clone());

    {
        let mut f = filtered_lines.write().await;
        *f = all_lines.clone().read().await.clone();
    }

    let list_state = ListState::default();

    enable_raw_mode()?;
    let mut screen = io::stderr();
    execute!(screen, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(screen);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    let _ = input_send.send(None);
    handle_input(ui_send, input_send, movement_send);
    process_input(input_recv, processed_send, &all_lines);
    render(&all_lines, terminal, list_state, processed_recv, ui_recv, movement_recv);
    futures::future::pending::<()>().await;
    Ok(())
}
