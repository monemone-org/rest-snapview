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

/// CLI configuration
struct CliConfig
{
    log_file: Option<String>,
}

fn parse_args() -> CliConfig
{
    let args: Vec<String> = std::env::args().collect();
    let mut config = CliConfig { log_file: None };

    let mut i = 1;
    while i < args.len()
    {
        match args[i].as_str()
        {
            "--log-file" | "-l" =>
            {
                if i + 1 < args.len()
                {
                    config.log_file = Some(args[i + 1].clone());
                    i += 2;
                }
                else
                {
                    eprintln!("Error: --log-file requires a path argument");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" =>
            {
                println!("rest-snapview - Terminal UI for browsing restic snapshots");
                println!();
                println!("Usage: rest-snapview [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -l, --log-file <PATH>  Save command logs to file");
                println!("  -h, --help             Show this help message");
                println!();
                println!("Environment variables:");
                println!("  RESTIC_REPOSITORY      Repository location (required)");
                println!("  RESTIC_PASSWORD        Repository password");
                println!("  RESTIC_PASSWORD_FILE   Path to password file");
                println!("  RESTIC_PASSWORD_COMMAND Command to get password");
                std::process::exit(0);
            }
            arg =>
            {
                eprintln!("Error: Unknown argument: {}", arg);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    }

    config
}

/// Results from background tasks
enum TaskResult
{
    Files
    {
        command: String,
        result: Result<Vec<FileNode>, String>,
        error_output: Option<String>,
    },
    Download
    {
        command: String,
        result: Result<String, String>,  // Ok(target path) or Err(error message)
        error_output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()>
{
    // Parse CLI arguments
    let config = parse_args();

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
    app.log_file_path = config.log_file;

    // Load initial snapshots
    let cmd_result = client.list_snapshots().await;
    app.add_command_log(
        cmd_result.command.clone(),
        cmd_result.result.is_ok(),
        cmd_result.error_output.clone(),
    );

    match cmd_result.result
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
        Command::NavigateDir { path } =>
        {
            if let Some(ref snapshot_id) = app.current_snapshot_id
            {
                let client = client.clone();
                let snapshot_id = snapshot_id.clone();
                tokio::spawn(async move {
                    let cmd_result = client.list_files(&snapshot_id, &path).await;
                    let task_result = TaskResult::Files {
                        command: cmd_result.command,
                        result: cmd_result.result
                            .map_err(|e| format!("Failed to list files: {}", e)),
                        error_output: cmd_result.error_output,
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
                    let cmd_result = client.restore(&snapshot_id, &path, &target_clone).await;
                    let task_result = TaskResult::Download {
                        command: cmd_result.command,
                        result: cmd_result.result
                            .map(|_| target_clone)
                            .map_err(|e| format!("Download failed: {}", e)),
                        error_output: cmd_result.error_output,
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
        TaskResult::Files { command, result, error_output } =>
        {
            app.add_command_log(command, result.is_ok(), error_output);
            match result
            {
                Ok(files) => app.set_files(files),
                Err(e) => app.set_error(e),
            }
        }
        TaskResult::Download { command, result, error_output } =>
        {
            app.add_command_log(command, result.is_ok(), error_output);
            match result
            {
                Ok(target) =>
                {
                    app.state = AppState::Ready;
                    app.set_status(format!("Downloaded to: {}", target));
                }
                Err(e) => app.set_error(e),
            }
        }
    }
}
