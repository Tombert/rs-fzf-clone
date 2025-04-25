use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::disable_raw_mode ;


use crossterm::execute;
use crossterm::terminal::LeaveAlternateScreen;

use rayon::prelude::*;
use tokio::sync::RwLock;
use std::io::{self};
use std::sync::Arc;

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

pub async fn do_filter(
    filtered_lines: &Arc<RwLock< Vec<(String, Vec<usize>)>>>,
    all_lines: &Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    input: &Arc<RwLock<String>>,
    selected: &mut Option<usize>,
) {
    {

        let mut f = filtered_lines.write().await;
        f.clear();
    }

    for (line, a) in &*all_lines.read().await {
        let res = fuzzy_search(input.read().await.as_str(), line.as_str()); 
        if let Some(s) = res {
            filtered_lines.write().await.push(s);
        }


    }
    // let mut filtered_lines2: Vec<(String, Vec<usize>)> = 
    //     all_lines.read().await
    //     .par_iter()
    //     .filter_map(|(line, _)| fuzzy_search(&input, line))
    //     .collect();

    {
        let mut f = filtered_lines.write().await; 
        f.par_sort_by_key(|(_, hits)| get_delta(hits));
        f.reverse();
        f.truncate(100);
    }
    // //filtered_lines2.reverse();
    //
    // *filtered_lines = filtered_lines2;

    if !filtered_lines.read().await.is_empty() {
        *selected = Some(filtered_lines.read().await.len() - 1);

    } else {
        *selected = None; 

    }

    // if !filtered_lines.is_empty() {
    //     *selected = Some(filtered_lines.len() - 1);
    // } else {
    //     *selected = None;
    // }
}

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
    BackSpace,
    Other,
    Key(char)
}

pub async fn do_handle(
    cursor_position: Arc<RwLock<usize>>,
    input: Arc<RwLock<String>>,
    filtered_lines:  Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    all_lines: Arc<RwLock<Vec<(String, Vec<usize>)>>>,
    selected: &mut Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    match event::read()? {
        Event::Key(key) => {
            let action = match key.code {
                KeyCode::Backspace => Action::BackSpace,
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
                KeyCode::Char(c) => Action::Key(c),
                _ => Action::Other,
            };

            match action {
                Action::Key(c) => {
                    if *cursor_position.read().await <= input.read().await.len() {
                        input.write().await.insert(*cursor_position.read().await, c);
                        *cursor_position.write().await += 1; 
                    }
                    do_filter(&filtered_lines.clone(), &all_lines.clone(), &input, selected).await;

                },
                Action::BackSpace => {
                    if *cursor_position.read().await > 0 {
                        input.write().await.remove(*cursor_position.read().await - 1);
                         *cursor_position.write().await -= 1; 
                    }
                    do_filter(&filtered_lines.clone(), &all_lines.clone(), &input, selected).await;
                },
                Action::ClearAll => {
                    *cursor_position.write().await = 0;
                    input.write().await.clear();
                    do_filter(&filtered_lines.clone(), &all_lines.clone(), &input, selected).await;
                }
                Action::Select => {
                    if let Some(sel) = selected {
                        if let Some(line) = filtered_lines.read().await.get(*sel) {
                            disable_raw_mode()?;
                            execute!(io::stderr(), LeaveAlternateScreen)?;
                            println!("{}", line.0);
                            std::process::exit(0);
                        }
                    }
                }
                Action::Exit => {
                    disable_raw_mode()?;
                    execute!(io::stderr(), LeaveAlternateScreen)?;
                    std::process::exit(0);
                }
                Action::MoveBegin => {
                    *cursor_position.write().await = 0;
                }
                Action::MoveEnd => {
                    *cursor_position.write().await = input.read().await.len();
                }
                Action::MoveLeft => {
                    if *cursor_position.read().await > 0 {
                        *cursor_position.write().await -= 1;
                    }
                }
                Action::MoveRight => {
                    if *cursor_position.read().await < input.read().await.len() {
                        *cursor_position.write().await += 1;
                    }
                }
                Action::MoveUp => {
                    if let Some(new_selected) = selected {
                        let ns = new_selected.clone();
                        if ns > 0 {
                            *selected = Some(ns - 1);
                        }
                    }
                }
                Action::MoveDown => {
                    if let Some(new_selected) = selected {
                        let ns = new_selected.clone();
                        if ns + 1 < filtered_lines.read().await.len() {
                            *selected = Some(ns + 1);
                        }
                    }
                }
                Action::Other => () 
            }
        }
        _ => {},
    }
    Ok(())
}

