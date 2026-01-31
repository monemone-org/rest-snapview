use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::event::{
    self, Command, Movement, is_back, is_download, is_help, is_panel_switch, is_quit, is_select,
};
use crate::file::{FileNode, parent_entry};
use crate::snapshot::Snapshot;

/// Which panel is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel
{
    Snapshots,
    Files,
}

/// Application state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState
{
    Loading,
    Ready,
    FileSearch,                  // Searching/filtering files
    DownloadDialog,              // Showing download directory picker
    Downloading(String),         // path being downloaded
    Error(String),
    Help,
}

/// Which control is focused in download dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogFocus
{
    PathPicker,      // Combined text input + directory list
    DownloadButton,
    CancelButton,
}

/// Download dialog state
pub struct DownloadDialog
{
    /// Source file path to download
    pub source_path: String,
    /// Current text in path input
    pub input_text: String,
    /// Cursor position in input text
    pub cursor_pos: usize,
    /// Directory entries for current path
    pub entries: Vec<DirEntry>,
    /// Selected entry index
    pub selected: usize,
    /// Scroll offset
    pub scroll: usize,
    /// Which control is focused
    pub focus: DialogFocus,
}

/// Simple directory entry for the picker
#[derive(Debug, Clone)]
pub struct DirEntry
{
    pub name: String,
    pub is_dir: bool,
}

impl DownloadDialog
{
    pub fn new(source_path: String,
               initial_dir: &str)
               -> Self
    {
        let mut dialog = Self {
            source_path,
            input_text: initial_dir.to_string(),
            cursor_pos: initial_dir.len(),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            focus: DialogFocus::PathPicker,
        };
        dialog.refresh_entries();
        dialog
    }

    /// Move focus to next control (Tab)
    pub fn focus_next(&mut self)
    {
        self.focus = match self.focus
        {
            DialogFocus::PathPicker => DialogFocus::DownloadButton,
            DialogFocus::DownloadButton => DialogFocus::CancelButton,
            DialogFocus::CancelButton => DialogFocus::PathPicker,
        };
    }

    /// Move focus to previous control (Shift+Tab)
    pub fn focus_prev(&mut self)
    {
        self.focus = match self.focus
        {
            DialogFocus::PathPicker => DialogFocus::CancelButton,
            DialogFocus::DownloadButton => DialogFocus::PathPicker,
            DialogFocus::CancelButton => DialogFocus::DownloadButton,
        };
    }

    /// Expand ~ to home directory
    fn expand_tilde(path: &str) -> String
    {
        if path.starts_with('~')
        {
            if let Some(home) = std::env::var_os("HOME")
            {
                let home_str = home.to_string_lossy();
                if path == "~"
                {
                    return home_str.to_string();
                }
                else if path.starts_with("~/")
                {
                    return format!("{}{}", home_str, &path[1..]);
                }
            }
        }
        path.to_string()
    }

    /// Refresh directory entries based on current input path
    pub fn refresh_entries(&mut self)
    {
        self.entries.clear();
        self.selected = 0;
        self.scroll = 0;

        let expanded = Self::expand_tilde(&self.input_text);
        let path = PathBuf::from(&expanded);
        let dir_to_read = if path.is_dir()
        {
            path.clone()
        }
        else
        {
            path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
        };

        // Add ".." entry if not at root
        if dir_to_read.parent().is_some() && dir_to_read.to_string_lossy() != "/"
        {
            self.entries.push(DirEntry {
                name: "..".to_string(),
                is_dir: true,
            });
        }

        if let Ok(read_dir) = std::fs::read_dir(&dir_to_read)
        {
            let mut entries: Vec<DirEntry> = read_dir
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let is_dir = e.file_type().ok()?.is_dir();
                    // Only show directories
                    if !is_dir
                    {
                        return None;
                    }
                    let name = e.file_name().to_string_lossy().to_string();
                    // Skip hidden files
                    if name.starts_with('.')
                    {
                        return None;
                    }
                    Some(DirEntry { name, is_dir })
                })
                .collect();

            entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            self.entries.extend(entries);
        }
    }

    /// Handle character input
    pub fn insert_char(&mut self,
                       c: char)
    {
        self.input_text.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.refresh_entries();
    }

    /// Handle backspace (delete char before cursor)
    pub fn backspace(&mut self)
    {
        if self.cursor_pos > 0
        {
            self.cursor_pos -= 1;
            self.input_text.remove(self.cursor_pos);
            self.refresh_entries();
        }
    }

    /// Handle delete
    pub fn delete(&mut self)
    {
        if self.cursor_pos < self.input_text.len()
        {
            self.input_text.remove(self.cursor_pos);
            self.refresh_entries();
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self)
    {
        if self.cursor_pos > 0
        {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self)
    {
        if self.cursor_pos < self.input_text.len()
        {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start
    pub fn cursor_home(&mut self)
    {
        self.cursor_pos = 0;
    }

    /// Move cursor to end
    pub fn cursor_end(&mut self)
    {
        self.cursor_pos = self.input_text.len();
    }

    /// Move selection up
    pub fn select_prev(&mut self)
    {
        if self.selected > 0
        {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self)
    {
        if !self.entries.is_empty() && self.selected < self.entries.len() - 1
        {
            self.selected += 1;
        }
    }

    /// Navigate into selected directory
    pub fn enter_selected(&mut self)
    {
        if let Some(entry) = self.entries.get(self.selected)
        {
            if entry.is_dir
            {
                // Handle ".." specially - go to parent
                if entry.name == ".."
                {
                    self.go_parent();
                    return;
                }

                // Expand ~ before processing
                let expanded = Self::expand_tilde(&self.input_text);
                let current = PathBuf::from(&expanded);
                let base = if current.is_dir()
                {
                    current
                }
                else
                {
                    current.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
                };
                let new_path = base.join(&entry.name);
                self.input_text = new_path.to_string_lossy().to_string();
                self.cursor_pos = self.input_text.len();
                self.refresh_entries();
            }
        }
    }

    /// Go to parent directory
    pub fn go_parent(&mut self)
    {
        // Expand ~ before processing
        let expanded = Self::expand_tilde(&self.input_text);
        let path = PathBuf::from(&expanded);
        let parent = if path.is_dir()
        {
            path.parent()
        }
        else
        {
            path.parent().and_then(|p| p.parent())
        };

        if let Some(parent) = parent
        {
            self.input_text = parent.to_string_lossy().to_string();
            if self.input_text.is_empty()
            {
                self.input_text = "/".to_string();
            }
            self.cursor_pos = self.input_text.len();
            self.refresh_entries();
        }
    }

    /// Get the confirmed download path (with ~ expanded)
    pub fn confirmed_path(&self) -> String
    {
        let expanded = Self::expand_tilde(&self.input_text);
        let path = PathBuf::from(&expanded);
        if path.is_dir()
        {
            expanded
        }
        else
        {
            path.parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string())
        }
    }

    /// Adjust scroll for visible area
    pub fn adjust_scroll(&mut self,
                         visible_height: usize)
    {
        if visible_height == 0
        {
            return;
        }

        if self.selected < self.scroll
        {
            self.scroll = self.selected;
        }
        else if self.selected >= self.scroll + visible_height
        {
            self.scroll = self.selected - visible_height + 1;
        }
    }
}

/// Spinner frames for loading animation
const SPINNER_FRAMES: &[char] = &[ '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏' ];

/// Cached state for a directory (for navigation stack)
#[derive(Clone)]
pub struct DirCache
{
    pub path: String,
    pub files: Vec<FileNode>,
    pub cursor: usize,
    pub scroll: usize,
}

/// Main application struct
pub struct App
{
    pub state: AppState,
    pub focused_panel: Panel,

    // Snapshots panel
    pub snapshots: Vec<Snapshot>,
    pub snapshot_cursor: usize,
    pub snapshot_scroll: usize,

    // Files panel
    pub current_snapshot_id: Option<String>,
    pub current_path: String,
    pub files: Vec<FileNode>,           // All files (unfiltered)
    pub filtered_files: Vec<usize>,     // Indices into files that match search
    pub file_cursor: usize,             // Cursor in filtered list
    pub file_scroll: usize,

    // Navigation stack (for back navigation without re-fetching)
    pub nav_stack: Vec<DirCache>,

    // File search
    pub search_query: String,
    pub search_cursor: usize,           // Cursor position in search input

    // Download dialog
    pub download_dialog: Option<DownloadDialog>,
    pub last_download_dir: String,

    // Status message
    pub status_message: Option<String>,

    // Spinner state
    pub spinner_frame: usize,

    // Visible heights for scroll calculations (updated by UI)
    pub snapshot_visible_height: usize,
    pub file_visible_height: usize,

    pub should_quit: bool,
}

impl App
{
    pub fn new() -> Self
    {
        // Default to current directory
        let default_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/".to_string());

        Self {
            state: AppState::Loading,
            focused_panel: Panel::Snapshots,
            snapshots: Vec::new(),
            snapshot_cursor: 0,
            snapshot_scroll: 0,
            current_snapshot_id: None,
            current_path: String::new(),
            files: Vec::new(),
            filtered_files: Vec::new(),
            file_cursor: 0,
            file_scroll: 0,
            nav_stack: Vec::new(),
            search_query: String::new(),
            search_cursor: 0,
            download_dialog: None,
            last_download_dir: default_dir,
            status_message: None,
            spinner_frame: 0,
            snapshot_visible_height: 20,
            file_visible_height: 20,
            should_quit: false,
        }
    }

    /// Advance spinner animation
    pub fn tick_spinner(&mut self)
    {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
    }

    /// Get current spinner character
    pub fn spinner_char(&self) -> char
    {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Handle a key event and return an optional command to execute
    pub fn handle_key(&mut self,
                      key: KeyEvent)
                      -> Option<Command>
    {
        let code = key.code;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Handle download dialog keys separately
        if self.state == AppState::DownloadDialog
        {
            return self.handle_download_dialog_key(key);
        }

        // Handle file search keys separately
        if self.state == AppState::FileSearch
        {
            return self.handle_file_search_key(code);
        }

        // Handle global keys first
        if is_quit(code)
        {
            if self.state == AppState::Help
            {
                self.state = AppState::Ready;
                return None;
            }
            self.should_quit = true;
            return Some(Command::Quit);
        }

        if is_help(code)
        {
            self.state = if self.state == AppState::Help
            {
                AppState::Ready
            }
            else
            {
                AppState::Help
            };
            return None;
        }

        // Don't process keys in help or loading state
        if matches!(self.state, AppState::Help | AppState::Loading | AppState::Downloading(_))
        {
            return None;
        }

        // Clear error state on any key
        if let AppState::Error(_) = &self.state
        {
            self.state = AppState::Ready;
        }

        // Handle movement (including vi-style Ctrl keys)
        if let Some(movement) = event::key_to_movement(&key)
        {
            self.apply_movement(movement);
            return None;
        }

        // Handle panel switch
        if is_panel_switch(code)
        {
            self.switch_panel();
            return None;
        }

        // Handle selection
        if is_select(code)
        {
            return self.select_item();
        }

        // Handle back navigation
        if is_back(code)
        {
            return self.go_back();
        }

        // Handle download (only without Ctrl, since Ctrl-D is half-page down)
        if !ctrl && is_download(code)
        {
            return self.open_download_dialog();
        }

        // Handle search (/ key in Files panel)
        if code == KeyCode::Char('/') && self.focused_panel == Panel::Files
        {
            self.start_file_search();
            return None;
        }

        None
    }

    /// Apply a movement to the current panel
    fn apply_movement(&mut self,
                      movement: Movement)
    {
        let (count, visible_height) = match self.focused_panel
        {
            Panel::Snapshots => (self.snapshots.len(), self.snapshot_visible_height),
            Panel::Files => (self.visible_file_count(), self.file_visible_height),
        };

        if count == 0
        {
            return;
        }

        let max = count - 1;
        let delta: i32 = match movement
        {
            Movement::Up(n) => -(n as i32),
            Movement::Down(n) => n as i32,
            Movement::PageUp => -(visible_height as i32),
            Movement::PageDown => visible_height as i32,
            Movement::HalfPageUp => -(visible_height as i32 / 2).max(1),
            Movement::HalfPageDown => (visible_height as i32 / 2).max(1),
            Movement::Top => i32::MIN,
            Movement::Bottom => i32::MAX,
        };

        let cursor = match self.focused_panel
        {
            Panel::Snapshots => &mut self.snapshot_cursor,
            Panel::Files => &mut self.file_cursor,
        };

        *cursor = Self::clamp_cursor(*cursor, delta, max);
    }

    /// Start file search mode
    fn start_file_search(&mut self)
    {
        if self.files.is_empty()
        {
            return;
        }
        self.search_query.clear();
        self.search_cursor = 0;
        self.apply_search_filter();
        self.state = AppState::FileSearch;
    }

    /// Handle key events in file search mode
    fn handle_file_search_key(&mut self,
                               key: KeyCode)
                               -> Option<Command>
    {
        match key
        {
            // Cancel search, clear filter
            KeyCode::Esc =>
            {
                self.search_query.clear();
                self.apply_search_filter();
                self.state = AppState::Ready;
            }

            // Confirm search, keep filter active
            KeyCode::Enter =>
            {
                self.state = AppState::Ready;
            }

            // Navigate filtered list
            KeyCode::Up | KeyCode::Char('k') =>
            {
                self.apply_movement(Movement::Up(1));
            }
            KeyCode::Down | KeyCode::Char('j') =>
            {
                self.apply_movement(Movement::Down(1));
            }

            // Text editing
            KeyCode::Backspace =>
            {
                if self.search_cursor > 0
                {
                    self.search_cursor -= 1;
                    self.search_query.remove(self.search_cursor);
                    self.apply_search_filter();
                }
            }
            KeyCode::Delete =>
            {
                if self.search_cursor < self.search_query.len()
                {
                    self.search_query.remove(self.search_cursor);
                    self.apply_search_filter();
                }
            }
            KeyCode::Left =>
            {
                if self.search_cursor > 0
                {
                    self.search_cursor -= 1;
                }
            }
            KeyCode::Right =>
            {
                if self.search_cursor < self.search_query.len()
                {
                    self.search_cursor += 1;
                }
            }
            KeyCode::Home =>
            {
                self.search_cursor = 0;
            }
            KeyCode::End =>
            {
                self.search_cursor = self.search_query.len();
            }

            // Character input
            KeyCode::Char(c) =>
            {
                self.search_query.insert(self.search_cursor, c);
                self.search_cursor += 1;
                self.apply_search_filter();
            }

            _ => {}
        }

        None
    }

    /// Apply search filter to files
    fn apply_search_filter(&mut self)
    {
        self.filtered_files.clear();
        self.file_cursor = 0;
        self.file_scroll = 0;

        let query = self.search_query.to_lowercase();

        for (i, file) in self.files.iter().enumerate()
        {
            // Always include ".." entry
            if file.name == ".."
            {
                self.filtered_files.push(i);
                continue;
            }

            // Match if query is empty or name contains query (case-insensitive)
            if query.is_empty() || file.name.to_lowercase().contains(&query)
            {
                self.filtered_files.push(i);
            }
        }
    }

    /// Get the currently visible files (filtered or all)
    pub fn visible_files(&self) -> Vec<&FileNode>
    {
        if self.search_query.is_empty() && self.state != AppState::FileSearch
        {
            self.files.iter().collect()
        }
        else
        {
            self.filtered_files
                .iter()
                .filter_map(|&i| self.files.get(i))
                .collect()
        }
    }

    /// Get file at cursor position (respecting filter)
    pub fn file_at_cursor(&self) -> Option<&FileNode>
    {
        if self.search_query.is_empty() && self.state != AppState::FileSearch
        {
            self.files.get(self.file_cursor)
        }
        else
        {
            self.filtered_files
                .get(self.file_cursor)
                .and_then(|&i| self.files.get(i))
        }
    }

    /// Handle key events in download dialog
    fn handle_download_dialog_key(&mut self,
                                   key: KeyEvent)
                                   -> Option<Command>
    {
        let dialog = match &mut self.download_dialog
        {
            Some(d) => d,
            None => return None,
        };

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Global keys (work regardless of focus)
        match key.code
        {
            // Esc: cancel dialog
            KeyCode::Esc =>
            {
                self.download_dialog = None;
                self.state = AppState::Ready;
                return None;
            }

            // Tab / Shift+Tab: cycle focus
            KeyCode::Tab | KeyCode::BackTab =>
            {
                if shift || key.code == KeyCode::BackTab
                {
                    dialog.focus_prev();
                }
                else
                {
                    dialog.focus_next();
                }
                return None;
            }

            _ => {}
        }

        // Focus-specific keys
        match dialog.focus
        {
            DialogFocus::PathPicker =>
            {
                match (key.code, ctrl)
                {
                    // Navigate directory listing
                    (KeyCode::Down, _) => dialog.select_next(),
                    (KeyCode::Up, _) => dialog.select_prev(),

                    // Enter (without Ctrl): navigate into selected directory
                    (KeyCode::Enter, false) => dialog.enter_selected(),

                    // Text cursor movement
                    (KeyCode::Left, _) => dialog.cursor_left(),
                    (KeyCode::Right, _) => dialog.cursor_right(),
                    (KeyCode::Home, _) => dialog.cursor_home(),
                    (KeyCode::End, _) => dialog.cursor_end(),

                    // Text editing
                    (KeyCode::Backspace, _) => dialog.backspace(),
                    (KeyCode::Delete, _) => dialog.delete(),
                    (KeyCode::Char(c), false) => dialog.insert_char(c),

                    _ => {}
                }
            }

            DialogFocus::DownloadButton =>
            {
                if key.code == KeyCode::Enter
                {
                    let target = dialog.confirmed_path();
                    let source = dialog.source_path.clone();
                    self.last_download_dir = target.clone();
                    self.download_dialog = None;
                    return Some(Command::Download {
                        path: source,
                        target,
                    });
                }
            }

            DialogFocus::CancelButton =>
            {
                if key.code == KeyCode::Enter
                {
                    self.download_dialog = None;
                    self.state = AppState::Ready;
                }
            }
        }

        None
    }

    /// Open the download dialog
    fn open_download_dialog(&mut self) -> Option<Command>
    {
        if self.focused_panel != Panel::Files
        {
            return None;
        }

        if let Some(file) = self.file_at_cursor()
        {
            // Don't download ".." entry
            if file.name == ".."
            {
                return None;
            }

            let path = file.path.clone();
            self.download_dialog = Some(DownloadDialog::new(
                path,
                &self.last_download_dir,
            ));
            self.state = AppState::DownloadDialog;
        }

        None
    }

    /// Get count of visible files (respecting filter)
    fn visible_file_count(&self) -> usize
    {
        if self.search_query.is_empty() && self.state != AppState::FileSearch
        {
            self.files.len()
        }
        else
        {
            self.filtered_files.len()
        }
    }

    /// Adjust scroll offset to keep cursor visible
    pub fn adjust_scroll(&mut self,
                         panel: Panel,
                         visible_height: usize)
    {
        if visible_height == 0
        {
            return;
        }

        match panel
        {
            Panel::Snapshots =>
            {
                // Scroll up if cursor is above visible area
                if self.snapshot_cursor < self.snapshot_scroll
                {
                    self.snapshot_scroll = self.snapshot_cursor;
                }
                // Scroll down if cursor is below visible area
                else if self.snapshot_cursor >= self.snapshot_scroll + visible_height
                {
                    self.snapshot_scroll = self.snapshot_cursor - visible_height + 1;
                }
            }
            Panel::Files =>
            {
                if self.file_cursor < self.file_scroll
                {
                    self.file_scroll = self.file_cursor;
                }
                else if self.file_cursor >= self.file_scroll + visible_height
                {
                    self.file_scroll = self.file_cursor - visible_height + 1;
                }
            }
        }
    }

    fn clamp_cursor(current: usize,
                    delta: i32,
                    max: usize)
                    -> usize
    {
        if delta == i32::MIN
        {
            return 0;
        }
        if delta == i32::MAX
        {
            return max;
        }

        let new_pos = current as i32 + delta;
        new_pos.clamp(0, max as i32) as usize
    }

    /// Switch between panels
    fn switch_panel(&mut self)
    {
        self.focused_panel = match self.focused_panel
        {
            Panel::Snapshots => Panel::Files,
            Panel::Files => Panel::Snapshots,
        };
    }

    /// Select the current item
    fn select_item(&mut self) -> Option<Command>
    {
        match self.focused_panel
        {
            Panel::Snapshots =>
            {
                if let Some(snapshot) = self.snapshots.get(self.snapshot_cursor)
                {
                    let path = snapshot.primary_path().to_string();
                    self.current_snapshot_id = Some(snapshot.full_id.clone());
                    self.current_path = path.clone();
                    self.focused_panel = Panel::Files;
                    self.file_cursor = 0;
                    self.nav_stack.clear(); // Clear stack when switching snapshots
                    self.state = AppState::Loading;
                    return Some(Command::LoadSnapshot {
                        snapshot_id: snapshot.full_id.clone(),
                        path,
                    });
                }
            }
            Panel::Files =>
            {
                // Get file info first to avoid borrow issues
                let file_info = self.file_at_cursor().map(|f| {
                    (f.is_dir(), f.name == "..", f.path.clone())
                });

                if let Some((is_dir, is_parent, path)) = file_info
                {
                    if is_dir
                    {
                        // Handle ".." specially - use go_back instead
                        if is_parent
                        {
                            return self.go_back();
                        }

                        // Push current state to navigation stack
                        self.nav_stack.push(DirCache {
                            path: self.current_path.clone(),
                            files: self.files.clone(),
                            cursor: self.file_cursor,
                            scroll: self.file_scroll,
                        });

                        self.current_path = path.clone();
                        self.file_cursor = 0;
                        self.search_query.clear(); // Clear search when navigating
                        self.state = AppState::Loading;
                        return Some(Command::NavigateDir { path });
                    }
                }
            }
        }
        None
    }

    /// Navigate back (parent directory)
    fn go_back(&mut self) -> Option<Command>
    {
        if self.focused_panel != Panel::Files || self.current_snapshot_id.is_none()
        {
            return None;
        }

        // Try to pop from navigation stack first (instant, no fetch needed)
        if let Some(cached) = self.nav_stack.pop()
        {
            self.current_path = cached.path;
            self.files = cached.files;
            self.file_cursor = cached.cursor;
            self.file_scroll = cached.scroll;
            self.filtered_files.clear();
            self.search_query.clear();
            self.state = AppState::Ready;
            return None; // No command needed, we restored from cache
        }

        // No cache available, need to fetch
        let parent = parent_entry(&self.current_path);
        if parent.path == self.current_path
        {
            // Already at root
            return None;
        }

        self.current_path = parent.path.clone();
        self.file_cursor = 0;
        self.state = AppState::Loading;
        Some(Command::NavigateDir { path: parent.path })
    }


    /// Set files for the current view
    pub fn set_files(&mut self,
                     files: Vec<FileNode>)
    {
        // Add parent directory entry if not at root
        let snapshot_root = self.snapshots
                                .get(self.snapshot_cursor)
                                .map(|s| s.primary_path())
                                .unwrap_or("/");

        let mut display_files = files;

        if self.current_path != snapshot_root && !self.current_path.is_empty()
        {
            display_files.insert(0, parent_entry(&self.current_path));
        }

        self.files = display_files;
        self.filtered_files.clear();
        self.search_query.clear();
        self.search_cursor = 0;
        self.file_cursor = 0;
        self.file_scroll = 0;
        self.state = AppState::Ready;
    }

    /// Set error state
    pub fn set_error(&mut self,
                     message: String)
    {
        self.state = AppState::Error(message);
    }

    /// Set status message
    pub fn set_status(&mut self,
                      message: String)
    {
        self.status_message = Some(message);
    }

}

impl Default for App
{
    fn default() -> Self
    {
        Self::new()
    }
}
