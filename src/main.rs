use crossterm::event::{self, EnableMouseCapture };
use crossterm::terminal:: enable_raw_mode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use tokio::io::{AsyncBufReadExt, BufReader,  Stdin};
use crossterm::terminal::disable_raw_mode ;
use crossterm::terminal::LeaveAlternateScreen;
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
use tokio::sync::RwLock;
use std::io::{self, Stderr};
use std::sync::Arc;
use std::time::Duration;

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

fn stdin_reader(state: Arc<RwLock<Vec<(String, Vec<usize>)>>>, reader: BufReader<Stdin>) {
    let mut lines = reader.lines();
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            state.write().await.push((line, Vec::new())); 
        }
    });
}

fn render(

    all_lines: &Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    mut terminal: Terminal<CrosstermBackend<Stderr>>,
    mut list_state : ListState,
    mut new_data_chan : UnboundedReceiver<Vec<(String, Vec<usize>)>>,
    mut ui_chan : UnboundedReceiver<UIStuff>) 
{
    //let filtered_lines = filtered_lines.clone();
    //let input = input.clone() ;
    let z = all_lines.clone(); 
    tokio::spawn(async move {
        let mut filtered_lines : Vec<(String, Vec<usize>)>= Vec::new(); 
        let mut ui_stuff = None; 
        loop {



            (filtered_lines, ui_stuff) = tokio::select! {
                new_l = new_data_chan.recv() => {
                    (new_l.unwrap_or(Vec::new()), ui_stuff)
                }, 
                ui_new = ui_chan.recv() =>{
                    (filtered_lines, ui_new)
                }

            };

            let total_len = z.clone().read().await.len();
            tokio::task::block_in_place(|| {
                terminal.draw(|f| {
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
                        selected: None,
                        enter : false
                    });

                    let selected_display = ui.selected.unwrap_or(0) + 1; // 1-based indexing
                    let label = format!("[ {}/{} ]", selected_display, total_len);
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
//&my_vec[..my_vec.len().min(100)]
                    let (items_to_render, real_selected) = if filtered_lines.len() <= list_height {
                        // Not enough items to fill the view, so pad the top
                        let padded_items = (0..padding_rows)
                            .map(|_| ListItem::new(""))
                            .chain(
                                filtered_lines[..filtered_lines.len().min(100)]
                                .iter()
                                .map(|(line, hits)| styled_line(line, hits)),
                            )
                            .collect::<Vec<_>>();

                        let real_selected = ui.selected.map(|sel| sel + padding_rows);
                        (padded_items, real_selected)
                    } else {
                        // Too many items, so scroll normally from the top
                        let start_idx = filtered_lines.len() - list_height;
                        let items = filtered_lines
                            .par_iter()
                            .skip(start_idx)
                            .take(list_height)
                            .map(|(line, hits)| styled_line(line, hits))
                            .collect::<Vec<_>>();

                        let real_selected = ui.selected.map(|sel| sel.saturating_sub(start_idx));
                        (items, real_selected)
                    };

                    let list = List::new(items_to_render)
                        .block(Block::default().borders(Borders::NONE))
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

                    list_state.select(real_selected);
                    f.render_stateful_widget(list, chunks[0], &mut list_state);
                }).unwrap();
            }); 
            //tokio::time::sleep(Duration::from_millis(8)).await;

            //tokio::time::sleep(Duration::from_millis(16)).await;
        }
    });

}

#[derive(Clone, Eq, PartialEq)]
struct UIStuff {
    input : String,
    cursor_position : usize,
    selected: Option<usize>,
    enter: bool, 
}

fn process_input(mut in_chan : UnboundedReceiver<String>, out_chan : UnboundedSender<Vec<(String, Vec<usize>)>>,
    all_lines: &Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    ) {

    let all_lines = all_lines.clone(); 
    tokio::spawn(async move {
        loop {
            if let Some(r) = in_chan.recv().await {

                if r != String::new() {
                    let mut x : Vec<(String,Vec<usize>)> = 
                        all_lines
                        .read()
                        .await
                        .par_iter()
                        .filter_map(|(line,_)| helpers::fuzzy_search(r.as_str(), line.as_str()))
                        .collect();
                    x.sort_by_key(|(_,k)| helpers::get_delta(k));
                    x.reverse();
                    let x = x[..100.min(x.len())].to_vec();

                    let _ = out_chan.send(x);
                    tokio::task::yield_now().await;
                } else {
                    //let mut al = Vec::new();
                    let all_lines = all_lines.read().await; 
                    let al = all_lines[..100.min(all_lines.len())].to_vec();
                    let _ = out_chan.send(al);
                    tokio::task::yield_now().await;
                }
            }
        }
    });
}

fn handle_input(ui_out_chan : UnboundedSender<UIStuff>, process_chan : UnboundedSender<String> ) {
    tokio::spawn(async move {
        let mut last_ui = UIStuff {
            input: String::new(), 
            enter: false, 
            cursor_position: 0, 
            selected : None,

        };

        let mut current_ui = last_ui.clone(); 

        loop {
            if let Ok(_) = event::poll(Duration::from_millis(50)) { 
                let res = match event::read() {
                    Ok(e) => helpers::parse_action(e),
                    _ => helpers::Action::Other
                };
                match res {
                    helpers::Action::Key(c) => {
                        if current_ui.cursor_position <= current_ui.input.len() {
                            current_ui.input.insert(current_ui.cursor_position, c);
                            current_ui.cursor_position += 1; 
                        }

                    },
                    helpers::Action::BackSpace => {
                        if current_ui.cursor_position > 0 {
                            current_ui.input.remove(current_ui.cursor_position - 1);
                            current_ui.cursor_position -= 1; 
                        }
                    },
                    helpers::Action::ClearAll => {
                        current_ui.cursor_position = 0;
                        current_ui.input.clear();
                    }
                    helpers::Action::Select => {
                        ()
                    }
                    helpers::Action::Exit => {
                        disable_raw_mode();
                        execute!(io::stderr(), LeaveAlternateScreen);
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
                        if let Some(new_selected) = current_ui.selected.clone() {
                            let ns = new_selected.clone();
                            if ns > 0 {
                                current_ui.selected = Some(ns - 1);
                            }
                        }
                    }
                    helpers::Action::MoveDown => {
                        if let Some(new_selected) = current_ui.selected.clone() {
                            let ns = new_selected.clone();
                            current_ui.selected = None;
                        }
                    }
                    helpers::Action::Other => () 
                }

            }

            if current_ui != last_ui {
                last_ui = current_ui.clone();
                let _ = process_chan.send(current_ui.input.clone());
                let _ = ui_out_chan.send(current_ui.clone()); 
                tokio::task::yield_now().await;

            }
        }
    });
}

#[tokio::main()]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin(); // 
    let reader = BufReader::new(stdin);
    let all_lines = Arc::new(RwLock::new(Vec::new()));
    let filtered_lines = Arc::new(RwLock::new(Vec::new()));

    stdin_reader(all_lines.clone(), reader);

    {
        let mut f = filtered_lines.write().await; 
        *f = all_lines.clone().read().await.clone();
    }
    
    let mut selected = if !filtered_lines.read().await.is_empty() {
        Some(filtered_lines.read().await.len() - 1) // or just Some(0)
    } else {
        None
    };
    let list_state = ListState::default();

    enable_raw_mode()?;
    let mut screen = io::stderr();
    execute!(screen, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(screen);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    let (ui_send, ui_recv) = tokio::sync::mpsc::unbounded_channel::<UIStuff>();
    let (input_send, input_recv) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (processed_send, processed_recv) = tokio::sync::mpsc::unbounded_channel::<Vec<(String, Vec<usize>)>>();
    let _ = input_send.send(String::new()); 
    handle_input(ui_send, input_send);
    process_input(input_recv, processed_send, &all_lines);
    render(&all_lines, terminal, list_state, processed_recv, ui_recv);
    futures::future::pending::<()>().await;
    Ok(())
}
