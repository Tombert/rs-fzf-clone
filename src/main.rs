use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};

use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Terminal;
use std::io::{self, BufRead};
use std::time::Duration;
use regex::Regex;
use ratatui::widgets::Paragraph;
use ratatui::style::Color;
use ratatui::text::{Span,Spans, Text};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

fn fuzzy_regex(input: &str) -> Regex {
    let pattern = input
        .chars()
        .map(|c| regex::escape(&c.to_string()) + ".*")
        .collect::<String>();
    Regex::new(&format!("(?i){}", pattern)).unwrap_or_else(|_| Regex::new(".*").unwrap())
}

fn get_delta(input: &Vec<usize>) -> usize {
    let mut delta = 0; 
    if input.len() > 1 {
        for i in input.windows(2) {
            let first = i[0]; 
            let second = i[1]; 
            delta += second - first; 
        }
    }

    delta
}

fn fuzzy_search(input: &str, line : &str) -> Option<(String, Vec<usize>)>{
    let mut input_index = 0; 
    let input_chars: Vec<char> = input.chars().collect();
    let input_length = input.len();
    let mut counter = 0; 
    let mut hits : Vec<usize> = Vec::new();
    let line_length = line.len();
    if input_length > 0  {
        for (i, cc) in line.chars().enumerate() {

            let current = input_chars.get(input_index).unwrap().to_ascii_lowercase();

            if cc.to_ascii_lowercase() == current {
                input_index += 1; 
                hits.push(counter); 

                if input_index >= input_length {
                    break; 
                }
            } else if i == line_length - 1 {
                return None
            }

            counter += 1; 
        }
    }

    if input_index < input_length {
        return None;
    }
    return Some((line.to_string(), hits)); 
}

fn styled_line(line: &str, hits: &Vec<usize>) -> ListItem<'static> {
    let mut spans = Vec::with_capacity(line.len());
    for (i, c) in line.chars().enumerate() {
        if hits.contains(&i) {
            spans.push(Span::styled(
                c.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::raw(c.to_string()));
        }
    }
    ListItem::new(Text::from(vec![Spans::from(spans)]))
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let all_lines: Vec<(String, Vec<usize>)> = stdin.lock().lines().filter_map(Result::ok).map(|s| (s, Vec::new())).collect();
    let mut filtered_lines = all_lines.clone();
    let mut selected = if !filtered_lines.is_empty() {
        Some(filtered_lines.len() - 1) // or just Some(0)
    } else {
        None
    };
    let mut list_state = ListState::default();
   // list_state.select(selected);
    let mut input = String::new();
    //list_state.select(Some(selected));

    enable_raw_mode()?;
    let mut screen = io::stderr();
    execute!(screen, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(screen);
    let mut terminal = Terminal::new(backend)?;
    let mut cursor_position = 0; 

    terminal.clear()?;
    loop {
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Min(1),Constraint::Length(1),  Constraint::Length(3)].as_ref())
                .split(size);


            let total = filtered_lines.len();
            let selected_display = selected.unwrap_or(0) + 1; // show 1-based indexing
            let label = format!("[ {}/{} ]", selected_display, total);
            let label_width = label.len() as u16;
            let divider_fill = if chunks[1].width > label_width {
                "─".repeat((chunks[1].width - label_width - 1) as usize)
            } else {
                "".to_string()
            };

            let divider_line = Paragraph::new(Spans::from(vec![
                    Span::styled(label, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(divider_fill, Style::default().fg(Color::DarkGray)),
            ]));
            f.render_widget(divider_line, chunks[1]);

            let input_para = Paragraph::new(Text::from(vec![
                    Spans::from(vec![
                        Span::styled("> ", Style::default().fg(Color::Blue)),
                        Span::raw(input.clone()),
                    ]),
            ]))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(input_para, chunks[2]);
            f.set_cursor(chunks[2].x + 2  + cursor_position as u16, chunks[2].y);

            let list_height = chunks[0].height as usize;
            let padding_rows = list_height.saturating_sub(filtered_lines.len());
            let real_selected = selected.map(|f| f + padding_rows);
            let mut items: Vec<ListItem> = Vec::with_capacity(list_height);
            for _ in 0..padding_rows {
                items.push(ListItem::new("")); // blank spacer rows
            }
            items.extend(
                filtered_lines
                .iter()
                .map(|(line, hits)| styled_line(line, hits))
            );

            let list = List::new(items)
                .block(Block::default().borders(Borders::NONE))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            list_state.select(real_selected);
            f.render_stateful_widget(list, chunks[0], &mut list_state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                   KeyCode::Char('b') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        if cursor_position > 0 {
                            cursor_position -= 1;
                        }
                    },
                   KeyCode::Char('f') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        if cursor_position < input.len() {
                            cursor_position += 1;
                        }
                    }
                   KeyCode::Right => {
                        if cursor_position < input.len() {
                            cursor_position += 1;
                        }
                    }
                   KeyCode::Left  => {
                        if cursor_position > 0 {
                            cursor_position -= 1;
                        }
                    },
                    KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if let Some(new_selected) = selected {
                                if new_selected > 0 {
                                    selected = Some(new_selected - 1);
                                }
                            }
                        }
                    KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            if let Some(new_selected) = selected {
                                if new_selected + 1 < filtered_lines.len() {
                                    selected = Some(new_selected + 1);
                                }
                            }
                        }
                    KeyCode::Char('a') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        cursor_position = 0; 
                        }
                    KeyCode::Char('e') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        cursor_position = input.len(); 
                        }
                    KeyCode::Char('c') => {
                        disable_raw_mode()?;
                        execute!(io::stderr(), LeaveAlternateScreen)?;
                        return Ok(());
                    }
                    KeyCode::Char(c) => {
                        if cursor_position <= input.len() {
                            input.insert(cursor_position, c);
                            cursor_position += 1;
                        }


                        //input.push(c);
                        let mut filtered_lines2: Vec<(String, Vec<usize>)> = all_lines
                            .iter()
                            .filter_map(|(line, _)| fuzzy_search(&input, line))
                            .collect();

                        filtered_lines2.sort_by_key(|(_, hits)| get_delta(hits));
                        filtered_lines2.reverse();

                        filtered_lines = filtered_lines2;


                        if !filtered_lines.is_empty() {
                            selected = Some(filtered_lines.len()-1);
                        } else {
                            selected = None;
                        }
                    }
                    KeyCode::Backspace => {
                        if cursor_position > 0 {
                            input.remove(cursor_position - 1);
                            cursor_position -= 1;
                        }

                        let mut filtered_lines2: Vec<(String, Vec<usize>)> = all_lines
                            .iter()
                            .filter_map(|(line, _)| fuzzy_search(&input, line))
                            .collect();

                        filtered_lines2.sort_by_key(|(_, hits)| get_delta(hits));
                        filtered_lines2.reverse();


                        filtered_lines = filtered_lines2;

                        if !filtered_lines.is_empty() {
                            selected = Some(filtered_lines.len()-1);
                        } else {
                            selected = None;
                        }
                    }
                    KeyCode::Up => {
                        if let Some(new_selected) = selected {
                            if new_selected > 0 {
                                selected = Some(new_selected - 1);
                                //list_state.select(selected);
                            }
                        }
                    }
                    KeyCode::Down => {
                        if let Some(new_selected) = selected {
                            if new_selected + 1 < filtered_lines.len() {
                                selected = Some(new_selected + 1);
                                //list_state.select(selected);
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(sel) = selected {
                            if let Some(line) = filtered_lines.get(sel) {
                                disable_raw_mode()?;
                                execute!(io::stderr(), LeaveAlternateScreen)?;
                                println!("{}", line.0);
                                return Ok(());
                            }
                        }
                    }
                    KeyCode::Esc => {
                        disable_raw_mode()?;
                        execute!(io::stderr(), LeaveAlternateScreen)?;
                        return Ok(());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}
