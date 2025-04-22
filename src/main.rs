use crossterm::event::{self, EnableMouseCapture, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};

use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::style::Color;
use ratatui::text::{Span, Line, Text};
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use rayon::prelude::*;
use std::io::{self, BufRead};
use std::time::Duration;

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

fn fuzzy_search(input: &str, line: &str) -> Option<(String, Vec<usize>)> {
    let mut input_index = 0;
    let input_chars: Vec<char> = input.chars().collect();
    let input_length = input.len();
    let mut counter = 0;
    let mut hits: Vec<usize> = Vec::new();
    let line_length = line.len();
    if input_length > 0 {
        for (i, cc) in line.chars().enumerate() {
            let current = input_chars.get(input_index).unwrap().to_ascii_lowercase();

            if cc.to_ascii_lowercase() == current {
                input_index += 1;
                hits.push(counter);

                if input_index >= input_length {
                    break;
                }
            } else if i == line_length - 1 {
                return None;
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

fn do_filter(
    filtered_lines: &mut Vec<(String, Vec<usize>)>,
    all_lines: &Vec<(String, Vec<usize>)>,
    input: &String,
    selected: &mut Option<usize>,
) {
    let mut filtered_lines2: Vec<(String, Vec<usize>)> = all_lines
        .par_iter()
        .filter_map(|(line, _)| fuzzy_search(&input, line))
        .collect();

    filtered_lines2.par_sort_by_key(|(_, hits)| get_delta(hits));
    filtered_lines2.reverse();

    *filtered_lines = filtered_lines2;

    if !filtered_lines.is_empty() {
        *selected = Some(filtered_lines.len() - 1);
    } else {
        *selected = None;
    }
}


fn do_handle(cursor_position : &mut usize, 
    input : &mut String,
    filtered_lines: &mut Vec<(String, Vec<usize>)>,
    all_lines: &Vec<(String, Vec<usize>)>,
    selected: &mut Option<usize>,
    ) -> Result<(), Box<dyn std::error::Error>> {
    match event::read()? {
        Event::Key(key) => {
            enum Action {
                MoveLeft,
                MoveRight,
                MoveUp,
                MoveDown,
                MoveEnd,
                MoveBegin,
                Exit,
                Select,
                ClearAll, 
                Other,
            }

            let action = match key.code {
                KeyCode::Enter => Action::Select,
                KeyCode::Esc => Action::Exit,
                KeyCode::Char('u')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::ClearAll
                    }
                KeyCode::Char('c')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::Exit
                    }
                KeyCode::Char('e')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveEnd
                    }
                KeyCode::Char('a')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveBegin
                    }

                KeyCode::Up => Action::MoveUp,
                KeyCode::Char('p')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveUp
                    }
                KeyCode::Down => Action::MoveDown,
                KeyCode::Char('n')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveDown
                    }
                KeyCode::Left => Action::MoveLeft,
                KeyCode::Char('b')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveLeft
                    }
                KeyCode::Right => Action::MoveRight,
                KeyCode::Char('f')
                    if key
                        .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        Action::MoveRight
                    }
                _ => Action::Other,
            };

            match action {
                Action::ClearAll => {
                    *cursor_position =  0;
                    input.clear();
                    do_filter(filtered_lines, &all_lines, &input, selected);
                    Ok(())
                },
                Action::Select => {
                    if let Some(sel) = selected {
                        if let Some(line) = filtered_lines.get(*sel) {
                            disable_raw_mode()?;
                            execute!(io::stderr(), LeaveAlternateScreen)?;
                            println!("{}", line.0);
                            std::process::exit(0);
                        }
                    }
                    Ok(())
                }
                Action::Exit => {
                    disable_raw_mode()?;
                    execute!(io::stderr(), LeaveAlternateScreen)?;
                    std::process::exit(0);
                }
                Action::MoveBegin => {
                    *cursor_position = 0;
                    Ok(())
                }
                Action::MoveEnd => {
                    *cursor_position = input.len();
                    Ok(())
                }
                Action::MoveLeft => {
                    if *cursor_position > 0 {
                        *cursor_position -= 1;
                    }
                    Ok(())
                }
                Action::MoveRight => {
                    if *cursor_position < input.len() {
                        *cursor_position += 1;
                    }
                    Ok(())
                }
                Action::MoveUp => {
                    if let Some(new_selected) = selected {
                        let ns = new_selected.clone(); 
                        if ns > 0 {
                            *selected =  Some(ns - 1);
                        }
                    }
                    Ok(())
                }
                Action::MoveDown => {
                    if let Some(new_selected) = selected {
                        let ns = new_selected.clone(); 
                        if ns + 1 < filtered_lines.len() {
                            *selected =  Some(ns + 1);
                        }
                    }
                    Ok(())
                }
                Action::Other => match key.code {
                    KeyCode::Char(c) => {
                        if *cursor_position <= input.len() {
                            input.insert(*cursor_position, c);
                            *cursor_position += 1;
                        }
                        do_filter(filtered_lines, &all_lines, &input, selected);
                    Ok(())
                    }
                    KeyCode::Backspace => {
                        if *cursor_position > 0 {
                            input.remove(*cursor_position - 1);
                            *cursor_position -= 1;
                        }
                        do_filter(filtered_lines, &all_lines, &input, selected);
                    Ok(())
                    }
                    _ => Ok(())
                },
            }
        }
        _ => Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let all_lines: Vec<(String, Vec<usize>)> = stdin
        .lock()
        .lines()
        .filter_map(Result::ok)
        .map(|s| (s, Vec::new()))
        .collect();
    let total_len = all_lines.len();
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
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Length(1),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let selected_display = selected.unwrap_or(0) + 1; // 1-based indexing
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
                Span::raw(input.clone()),
            ])]))
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(input_para, chunks[2]);
            f.set_cursor(chunks[2].x + 2 + cursor_position as u16, chunks[2].y);

            let list_height = chunks[0].height as usize;
            let actual_items_to_show = filtered_lines.len().min(list_height);

            let padding_rows = list_height.saturating_sub(actual_items_to_show);

            let (items_to_render, real_selected) = if filtered_lines.len() <= list_height {
                // Not enough items to fill the view, so pad the top
                let padded_items = (0..padding_rows)
                    .map(|_| ListItem::new(""))
                    .chain(
                        filtered_lines
                            .iter()
                            .map(|(line, hits)| styled_line(line, hits)),
                    )
                    .collect::<Vec<_>>();

                let real_selected = selected.map(|sel| sel + padding_rows);
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

                let real_selected = selected.map(|sel| sel.saturating_sub(start_idx));
                (items, real_selected)
            };

            let list = List::new(items_to_render)
                .block(Block::default().borders(Borders::NONE))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            list_state.select(real_selected);
            f.render_stateful_widget(list, chunks[0], &mut list_state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            let _ = do_handle(&mut cursor_position, &mut input, &mut filtered_lines, &all_lines, &mut selected);
        }
    }
}
