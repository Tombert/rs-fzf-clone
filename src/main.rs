use crossterm::event::{self, EnableMouseCapture };
use crossterm::terminal:: enable_raw_mode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};

use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use ratatui::Terminal;
use ratatui::style::Color;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
mod helpers;

use rayon::prelude::*;
use std::io::{self, BufRead};
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
    let mut input = String::new();

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
            let _ = helpers::do_handle(
                &mut cursor_position,
                &mut input,
                &mut filtered_lines,
                &all_lines,
                &mut selected,
            );
        }
    }
}
