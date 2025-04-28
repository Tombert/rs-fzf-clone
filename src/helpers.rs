use crate::types;
use crossterm::event::{Event, KeyCode};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::ListItem,
};

pub fn vec_insert_expand<T>(vec: &mut Vec<Option<Vec<T>>>, index: usize, value: T) {
    if vec.len() <= index {
        vec.resize_with(index + 1, || None);
    }
    vec[index].get_or_insert_with(Vec::new).push(value);
}

pub fn styled_line(line: &str, hits: &Vec<usize>) -> ListItem<'static> {
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

pub fn fuzzy_search(input: &str, line: &str) -> Option<(String, Vec<usize>)> {
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

pub fn get_delta(input: &Vec<usize>) -> usize {
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

pub fn parse_action(ev: Event) -> types::Action {
    match ev {
        Event::Key(key) => match key.code {
            KeyCode::Backspace => types::Action::BackSpace,
            KeyCode::Enter => types::Action::Select,
            KeyCode::Esc => types::Action::Exit,
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::ClearAll
            }
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::Exit
            }
            KeyCode::Char('e')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveEnd
            }
            KeyCode::Char('a')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveBegin
            }

            KeyCode::Up => types::Action::MoveUp,
            KeyCode::Char('p')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveUp
            }
            KeyCode::Down => types::Action::MoveDown,
            KeyCode::Char('n')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveDown
            }
            KeyCode::Left => types::Action::MoveLeft,
            KeyCode::Char('b')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveLeft
            }
            KeyCode::Right => types::Action::MoveRight,
            KeyCode::Char('f')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                types::Action::MoveRight
            }
            KeyCode::Char(c) => types::Action::Key(c),
            _ => types::Action::Other,
        },
        _ => types::Action::Other,
    }
}
