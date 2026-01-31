use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{App, AppState, DialogFocus, DownloadDialog, Panel};

/// Main render function
pub fn render(frame: &mut Frame,
              app: &mut App)
{
    let chunks = Layout::vertical([
        Constraint::Percentage(40), // Snapshots
        Constraint::Min(5),         // Files
        Constraint::Length(1),      // Status bar
    ])
    .split(frame.area());

    render_snapshots(frame, app, chunks[0]);
    render_files(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);

    // Render loading overlay if loading
    if matches!(app.state, AppState::Loading | AppState::Downloading(_))
    {
        render_loading_overlay(frame, app);
    }

    // Render download dialog
    if app.state == AppState::DownloadDialog
    {
        render_download_dialog(frame, app);
    }

    // Render help overlay if in help state
    if app.state == AppState::Help
    {
        render_help_overlay(frame);
    }
}

/// Render the snapshots panel
fn render_snapshots(frame: &mut Frame,
                    app: &mut App,
                    area: Rect)
{
    let focused = app.focused_panel == Panel::Snapshots;
    let border_style = if focused
    {
        Style::default().fg(Color::Cyan)
    }
    else
    {
        Style::default().fg(Color::DarkGray)
    };

    // Calculate visible height (area height minus borders)
    let visible_height = area.height.saturating_sub(2) as usize;

    // Save visible height for movement calculations
    app.snapshot_visible_height = visible_height;

    // Adjust scroll to keep cursor visible
    app.adjust_scroll(Panel::Snapshots, visible_height);

    let title = format!(" Snapshots ({}) ", app.snapshots.len());
    let block = Block::default().title(title)
                                .borders(Borders::ALL)
                                .border_style(border_style);

    if app.snapshots.is_empty()
    {
        let message = match &app.state
        {
            AppState::Loading => "Loading snapshots...",
            AppState::Error(e) => e.as_str(),
            _ => "No snapshots found",
        };
        let paragraph = Paragraph::new(message).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> =
        app.snapshots
           .iter()
           .enumerate()
           .skip(app.snapshot_scroll)
           .take(visible_height)
           .map(|(i, snapshot)| {
               let is_selected = i == app.snapshot_cursor;
               let prefix = if is_selected { ">" } else { " " };

               let line = format!("{} {:8}  {}  {}",
                                  prefix,
                                  snapshot.display_id(),
                                  snapshot.formatted_time(),
                                  snapshot.primary_path());

               let style = if is_selected && focused
               {
                   Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
               }
               else if is_selected
               {
                   Style::default().fg(Color::White)
               }
               else
               {
                   Style::default().fg(Color::Gray)
               };

               ListItem::new(line).style(style)
           })
           .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Render the files panel
fn render_files(frame: &mut Frame,
                app: &mut App,
                area: Rect)
{
    let focused = app.focused_panel == Panel::Files;
    let is_searching = app.state == AppState::FileSearch;
    let has_filter = !app.search_query.is_empty();

    let border_style = if focused || is_searching
    {
        Style::default().fg(Color::Cyan)
    }
    else
    {
        Style::default().fg(Color::DarkGray)
    };

    // Split area for search bar if searching
    let (search_area, list_area) = if is_searching || has_filter
    {
        let chunks = Layout::vertical([
            Constraint::Length(1), // Search bar
            Constraint::Min(3),    // File list
        ])
        .split(area);
        (Some(chunks[0]), chunks[1])
    }
    else
    {
        (None, area)
    };

    // Render search bar if visible
    if let Some(search_area) = search_area
    {
        render_search_bar(frame, app, search_area, is_searching);
    }

    // Calculate visible height
    let visible_height = list_area.height.saturating_sub(2) as usize;

    // Save visible height for movement calculations
    app.file_visible_height = visible_height;

    // Adjust scroll to keep cursor visible
    app.adjust_scroll(Panel::Files, visible_height);

    // Get visible files
    let visible_files = app.visible_files();
    let file_count = visible_files.len();
    let total_count = app.files.len();

    let title = if app.current_path.is_empty()
    {
        " Files ".to_string()
    }
    else if has_filter
    {
        format!(" Files ({}) [{}/{} matches] ", app.current_path, file_count, total_count)
    }
    else
    {
        format!(" Files ({}) [{} items] ", app.current_path, total_count)
    };

    let block = Block::default().title(title)
                                .borders(Borders::ALL)
                                .border_style(border_style);

    // Show loading or empty state
    if app.current_snapshot_id.is_none()
    {
        let paragraph = Paragraph::new("Select a snapshot to browse files").block(block);
        frame.render_widget(paragraph, list_area);
        return;
    }

    if app.files.is_empty()
    {
        let message = match &app.state
        {
            AppState::Loading => "Loading files...",
            AppState::Error(e) => e.as_str(),
            _ => "Empty directory",
        };
        let paragraph = Paragraph::new(message).block(block);
        frame.render_widget(paragraph, list_area);
        return;
    }

    if visible_files.is_empty()
    {
        let paragraph = Paragraph::new("  No matches found").block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, list_area);
        return;
    }

    let items: Vec<ListItem> =
        visible_files
           .iter()
           .enumerate()
           .skip(app.file_scroll)
           .take(visible_height)
           .map(|(i, file)| {
               let is_selected = i == app.file_cursor;
               let prefix = if is_selected { ">" } else { " " };

               // Format: "> name                                 [DIR] or size"
               let name_display = if file.is_dir() && file.name != ".."
               {
                   format!("{}/", file.name)
               }
               else
               {
                   file.name.clone()
               };

               let size_display = file.formatted_size();

               let line = format!("{} {:<50} {:>10}", prefix, name_display, size_display);

               let style = if is_selected && (focused || is_searching)
               {
                   Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
               }
               else if is_selected
               {
                   Style::default().fg(Color::White)
               }
               else if file.is_dir()
               {
                   Style::default().fg(Color::Blue)
               }
               else
               {
                   Style::default().fg(Color::Gray)
               };

               ListItem::new(line).style(style)
           })
           .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, list_area);
}

/// Render the search bar
fn render_search_bar(frame: &mut Frame,
                     app: &App,
                     area: Rect,
                     is_active: bool)
{
    let style = if is_active
    {
        Style::default().fg(Color::Yellow)
    }
    else
    {
        Style::default().fg(Color::DarkGray)
    };

    let search_text = format!("/{}",  app.search_query);
    let paragraph = Paragraph::new(search_text).style(style);
    frame.render_widget(paragraph, area);

    // Show cursor if actively searching
    if is_active
    {
        frame.set_cursor_position((area.x + 1 + app.search_cursor as u16, area.y));
    }
}

/// Render the status bar
fn render_status_bar(frame: &mut Frame,
                     app: &App,
                     area: Rect)
{
    let spinner = app.spinner_char();

    let status_text = if let Some(ref msg) = app.status_message
    {
        msg.clone()
    }
    else
    {
        match &app.state
        {
            AppState::Loading => format!("{} Loading...", spinner),
            AppState::Downloading(path) => format!("{} Downloading: {}", spinner, path),
            AppState::FileSearch => "[Enter]confirm  [Esc]clear  [↑↓]navigate".to_string(),
            AppState::DownloadDialog => "[Tab]switch  [↑↓]select  [Enter]open/confirm  [Esc]cancel".to_string(),
            AppState::Error(e) => format!("Error: {}", e),
            AppState::Help => "Press q or ? to close help".to_string(),
            AppState::Ready =>
            {
                "[↑↓/jk]move  [Tab]panel  [Enter]open  [Backspace]back  [d]download  [?]help  [q]uit"
                    .to_string()
            }
        }
    };

    let style = match &app.state
    {
        AppState::Error(_) => Style::default().fg(Color::Red),
        AppState::Loading | AppState::Downloading(_) => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::DarkGray),
    };

    let paragraph = Paragraph::new(status_text).style(style);
    frame.render_widget(paragraph, area);
}

/// Render loading overlay
fn render_loading_overlay(frame: &mut Frame,
                          app: &App)
{
    let area = centered_rect(40, 20, frame.area());

    frame.render_widget(Clear, area);

    let spinner = app.spinner_char();
    let message = match &app.state
    {
        AppState::Loading => format!("{}  Loading...", spinner),
        AppState::Downloading(path) =>
        {
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            format!("{}  Downloading: {}", spinner, filename)
        }
        _ => return,
    };

    let block = Block::default().borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Yellow));

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(message, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(text).block(block)
                                         .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render help overlay
fn render_help_overlay(frame: &mut Frame)
{
    let area = centered_rect(60, 70, frame.area());

    // Clear the area first
    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(vec![
            Span::styled("Keyboard Controls", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / k    ", Style::default().fg(Color::Cyan)),
            Span::raw("Move cursor up"),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j    ", Style::default().fg(Color::Cyan)),
            Span::raw("Move cursor down"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-F   ", Style::default().fg(Color::Cyan)),
            Span::raw("Page down (full screen)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-B   ", Style::default().fg(Color::Cyan)),
            Span::raw("Page up (full screen)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-D   ", Style::default().fg(Color::Cyan)),
            Span::raw("Scroll down (half screen)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-U   ", Style::default().fg(Color::Cyan)),
            Span::raw("Scroll up (half screen)"),
        ]),
        Line::from(vec![
            Span::styled("  g / Home ", Style::default().fg(Color::Cyan)),
            Span::raw("Go to first item"),
        ]),
        Line::from(vec![
            Span::styled("  G / End  ", Style::default().fg(Color::Cyan)),
            Span::raw("Go to last item"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab      ", Style::default().fg(Color::Cyan)),
            Span::raw("Switch panel"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(Color::Cyan)),
            Span::raw("Open directory / Select snapshot"),
        ]),
        Line::from(vec![
            Span::styled("  Bksp/h   ", Style::default().fg(Color::Cyan)),
            Span::raw("Go to parent directory"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(Color::Cyan)),
            Span::raw("Search/filter files (in Files panel)"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(Color::Cyan)),
            Span::raw("Download selected file/folder"),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(Color::Cyan)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc  ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Search Mode:", Style::default().fg(Color::Yellow)),
        ]),
        Line::from("  Type to filter, Enter=confirm, Esc=clear"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Download Dialog:", Style::default().fg(Color::Yellow)),
        ]),
        Line::from("  Tab/Shift+Tab=switch focus  Esc=cancel"),
        Line::from("  Path picker: type, ↑↓=select, Enter=open"),
        Line::from("  On button: Enter=activate"),
    ];

    let block = Block::default().title(" Help ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text).block(block);
    frame.render_widget(paragraph, area);
}

/// Render download directory picker dialog
fn render_download_dialog(frame: &mut Frame,
                          app: &mut App)
{
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    let dialog = match &mut app.download_dialog
    {
        Some(d) => d,
        None => return,
    };

    // Get source filename for title
    let source_name = std::path::Path::new(&dialog.source_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| dialog.source_path.clone());

    let block = Block::default()
        .title(format!(" Download: {} ", source_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(block, area);

    // Inner area (without borders)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Layout: path input (3 lines), directory listing (rest), buttons (3 lines)
    let chunks = Layout::vertical([
        Constraint::Length(3), // Path input
        Constraint::Min(3),    // Directory listing
        Constraint::Length(3), // Buttons
    ])
    .split(inner);

    // Render path input
    render_path_input(frame, dialog, chunks[0]);

    // Render directory listing
    render_dir_listing(frame, dialog, chunks[1]);

    // Render buttons
    render_dialog_buttons(frame, dialog, chunks[2]);
}

/// Render dialog buttons
fn render_dialog_buttons(frame: &mut Frame,
                         dialog: &DownloadDialog,
                         area: Rect)
{
    let download_focused = dialog.focus == DialogFocus::DownloadButton;
    let cancel_focused = dialog.focus == DialogFocus::CancelButton;

    let download_style = if download_focused
    {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    }
    else
    {
        Style::default().fg(Color::White)
    };

    let cancel_style = if cancel_focused
    {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    }
    else
    {
        Style::default().fg(Color::White)
    };

    let buttons = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("        "),
            Span::styled(" [ Download ] ", download_style),
            Span::raw("        "),
            Span::styled(" [ Cancel ] ", cancel_style),
            Span::raw("        "),
        ]),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(buttons).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render the path input box
fn render_path_input(frame: &mut Frame,
                     dialog: &DownloadDialog,
                     area: Rect)
{
    let is_focused = dialog.focus == DialogFocus::PathPicker;
    let border_color = if is_focused { Color::Yellow } else { Color::DarkGray };

    let block = Block::default()
        .title(" Target Directory ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let input_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };

    frame.render_widget(block, area);

    // Render input text with cursor
    let display_width = input_area.width as usize;
    let cursor_pos = dialog.cursor_pos;
    let text = &dialog.input_text;

    // Calculate visible window of text
    let (visible_text, cursor_x) = if text.len() <= display_width
    {
        (text.as_str(), cursor_pos)
    }
    else
    {
        // Scroll text to keep cursor visible
        let start = if cursor_pos < display_width / 2
        {
            0
        }
        else if cursor_pos > text.len() - display_width / 2
        {
            text.len().saturating_sub(display_width)
        }
        else
        {
            cursor_pos - display_width / 2
        };
        let end = (start + display_width).min(text.len());
        (&text[start..end], cursor_pos - start)
    };

    let paragraph = Paragraph::new(visible_text).style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, input_area);

    // Show cursor only when path picker is focused
    if is_focused
    {
        frame.set_cursor_position((input_area.x + cursor_x as u16, input_area.y));
    }
}

/// Render the directory listing
fn render_dir_listing(frame: &mut Frame,
                      dialog: &mut DownloadDialog,
                      area: Rect)
{
    let is_focused = dialog.focus == DialogFocus::PathPicker;
    let border_color = if is_focused { Color::Yellow } else { Color::DarkGray };

    let block = Block::default()
        .title(" Directories ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_height = area.height.saturating_sub(2) as usize;
    dialog.adjust_scroll(inner_height);

    if dialog.entries.is_empty()
    {
        let paragraph = Paragraph::new("  (no subdirectories)").block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = dialog
        .entries
        .iter()
        .enumerate()
        .skip(dialog.scroll)
        .take(inner_height)
        .map(|(i, entry)| {
            let is_selected = i == dialog.selected;
            let prefix = if is_selected { ">" } else { " " };
            let name = format!("{} {}/", prefix, entry.name);

            let style = if is_selected && is_focused
            {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            }
            else if is_selected
            {
                Style::default().fg(Color::White)
            }
            else
            {
                Style::default().fg(Color::Blue)
            };

            ListItem::new(name).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Create a centered rect with percentage of parent
fn centered_rect(percent_x: u16,
                 percent_y: u16,
                 area: Rect)
                 -> Rect
{
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
