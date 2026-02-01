use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Commands that result from user input
#[derive(Debug, Clone)]
pub enum Command
{
    /// Navigate into a directory
    NavigateDir
    {
        path: String
    },
    /// Download the selected file/directory
    Download
    {
        path: String,
        target: String,
    },
    /// Quit the application
    Quit,
}

/// Movement amount for vi-style navigation
#[derive(Debug, Clone, Copy)]
pub enum Movement
{
    Up(i32),
    Down(i32),
    PageUp,      // Full page up (Ctrl-B)
    PageDown,    // Full page down (Ctrl-F)
    HalfPageUp,  // Half page up (Ctrl-U)
    HalfPageDown,// Half page down (Ctrl-D)
    Top,         // Go to top (Home, gg)
    Bottom,      // Go to bottom (End, G)
}

/// Convert a key event to movement
pub fn key_to_movement(key: &KeyEvent) -> Option<Movement>
{
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match (key.code, ctrl)
    {
        // Vi-style Ctrl navigation
        (KeyCode::Char('f'), true) => Some(Movement::PageDown),
        (KeyCode::Char('b'), true) => Some(Movement::PageUp),
        (KeyCode::Char('d'), true) => Some(Movement::HalfPageDown),
        (KeyCode::Char('u'), true) => Some(Movement::HalfPageUp),

        // Standard navigation
        (KeyCode::Up, _) | (KeyCode::Char('k'), false) => Some(Movement::Up(1)),
        (KeyCode::Down, _) | (KeyCode::Char('j'), false) => Some(Movement::Down(1)),
        (KeyCode::PageUp, _) => Some(Movement::PageUp),
        (KeyCode::PageDown, _) => Some(Movement::PageDown),
        (KeyCode::Home, _) | (KeyCode::Char('g'), false) => Some(Movement::Top),
        (KeyCode::End, _) | (KeyCode::Char('G'), false) => Some(Movement::Bottom),

        _ => None,
    }
}


/// Check if key is a panel switch
pub fn is_panel_switch(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Tab | KeyCode::BackTab)
}

/// Check if key is a selection/enter
pub fn is_select(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Enter)
}

/// Check if key is go back
pub fn is_back(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h'))
}

/// Check if key is download
pub fn is_download(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Char('d'))
}

/// Check if key is quit
pub fn is_quit(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Char('q') | KeyCode::Esc)
}

/// Check if key is help
pub fn is_help(key: KeyCode) -> bool
{
    matches!(key, KeyCode::Char('?'))
}
