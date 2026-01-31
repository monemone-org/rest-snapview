mod app;
mod event;
mod file;
mod restic;
mod snapshot;
mod ui;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self as ct_event, Event};
use tokio::sync::mpsc;

use app::{App, AppState};
use crate::event::Command;
use crate::file::FileNode;
use restic::ResticClient;

/// Results from background tasks
enum TaskResult
{
    Files(Result<Vec<FileNode>, String>),
    Download(Result<String, String>), // Ok(target path) or Err(error message)
}

#[tokio::main]
async fn main() -> Result<()>
{
    // Create restic client from environment
    let client = match ResticClient::from_env()
    {
        Ok(c) => c,
        Err(e) =>
        {
            eprintln!("Error: {}", e);
            eprintln!();
            eprintln!("Required environment variables:");
            eprintln!("  RESTIC_REPOSITORY    - Repository location");
            eprintln!("  RESTIC_PASSWORD      - Repository password");
            eprintln!("  or RESTIC_PASSWORD_FILE - Path to password file");
            eprintln!();
            eprintln!("Example:");
            eprintln!("  export RESTIC_REPOSITORY=\"rest:https://your-server/repo\"");
            eprintln!("  export RESTIC_PASSWORD_FILE=\"$HOME/.restic-password\"");
            std::process::exit(1);
        }
    };

    // Initialize terminal
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // Create app
    let mut app = App::new();

    // Load initial snapshots
    match client.list_snapshots().await
    {
        Ok(snapshots) =>
        {
            app.snapshots = snapshots;
            app.state = AppState::Ready;
        }
        Err(e) =>
        {
            app.set_error(format!("Failed to load snapshots: {}", e));
        }
    }

    // Run event loop
    let result = run_event_loop(&mut terminal, &mut app, client).await;

    // Restore terminal
    ratatui::restore();

    result
}

async fn run_event_loop(terminal: &mut ratatui::DefaultTerminal,
                        app: &mut App,
                        client: ResticClient)
                        -> Result<()>
{
    // Channel for receiving results from background tasks
    let (tx, mut rx) = mpsc::channel::<TaskResult>(10);

    loop
    {
        // Tick spinner for animation
        app.tick_spinner();

        // Check for completed background tasks (non-blocking)
        while let Ok(result) = rx.try_recv()
        {
            handle_task_result(app, result);
        }

        // Draw UI
        terminal.draw(|frame| ui::render(frame, app))?;

        // Poll for events with short timeout to keep spinner animated
        if ct_event::poll(Duration::from_millis(80))?
        {
            if let Event::Key(key) = ct_event::read()?
            {
                // Handle key and get optional command
                if let Some(cmd) = app.handle_key(key)
                {
                    spawn_command(&client, cmd, tx.clone(), app);
                }
            }
        }

        if app.should_quit
        {
            break;
        }
    }

    Ok(())
}

/// Spawn a command as a background task
fn spawn_command(client: &ResticClient,
                 cmd: Command,
                 tx: mpsc::Sender<TaskResult>,
                 app: &mut App)
{
    match cmd
    {
        Command::LoadSnapshot { snapshot_id, path } =>
        {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client.list_files(&snapshot_id, &path).await;
                let task_result = match result
                {
                    Ok(files) => TaskResult::Files(Ok(files)),
                    Err(e) => TaskResult::Files(Err(format!("Failed to list files: {}", e))),
                };
                let _ = tx.send(task_result).await;
            });
        }
        Command::NavigateDir { path } =>
        {
            if let Some(ref snapshot_id) = app.current_snapshot_id
            {
                let client = client.clone();
                let snapshot_id = snapshot_id.clone();
                tokio::spawn(async move {
                    let result = client.list_files(&snapshot_id, &path).await;
                    let task_result = match result
                    {
                        Ok(files) => TaskResult::Files(Ok(files)),
                        Err(e) => TaskResult::Files(Err(format!("Failed to list files: {}", e))),
                    };
                    let _ = tx.send(task_result).await;
                });
            }
        }
        Command::Download { path, target } =>
        {
            if let Some(ref snapshot_id) = app.current_snapshot_id
            {
                // Set downloading state before spawning
                app.state = AppState::Downloading(path.clone());

                let client = client.clone();
                let snapshot_id = snapshot_id.clone();
                let target_clone = target.clone();
                tokio::spawn(async move {
                    let result = client.restore(&snapshot_id, &path, &target_clone).await;
                    let task_result = match result
                    {
                        Ok(()) => TaskResult::Download(Ok(target_clone)),
                        Err(e) => TaskResult::Download(Err(format!("Download failed: {}", e))),
                    };
                    let _ = tx.send(task_result).await;
                });
            }
        }
        Command::Quit =>
        {
            // Already handled by should_quit flag
        }
    }
}

/// Handle results from background tasks
fn handle_task_result(app: &mut App,
                      result: TaskResult)
{
    match result
    {
        TaskResult::Files(Ok(files)) =>
        {
            app.set_files(files);
        }
        TaskResult::Files(Err(e)) =>
        {
            app.set_error(e);
        }
        TaskResult::Download(Ok(target)) =>
        {
            app.state = AppState::Ready;
            app.set_status(format!("Downloaded to: {}", target));
        }
        TaskResult::Download(Err(e)) =>
        {
            app.set_error(e);
        }
    }
}
