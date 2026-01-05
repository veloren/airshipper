use crate::{
    Result, fs, gui, io,
    logger::{self, pretty_bytes},
    profiles::{Profile, parse_env_vars},
};
use parse::Action;
mod parse;
use iced::futures::stream::StreamExt;

use crate::{BASE_PATH, error::ClientError, profiles::LogLevel};
pub use parse::CmdLine;
use tracing::level_filters::LevelFilter;

/// Process command line arguments and optionally starts GUI
pub fn process() -> Result<()> {
    let mut cmd = CmdLine::new();

    let level = match cmd.debug {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        2 => LevelFilter::TRACE,
        _ => LevelFilter::TRACE,
    };

    let log = fs::log_path_file();
    let _guard = logger::init(Some((log.0, log.1)), level);

    tracing::debug!("Running on {}", std::env::consts::OS);
    tracing::debug!("Base Path: {}", fs::base_path());
    tracing::debug!("Log file: {}", fs::log_file().display());
    #[cfg(windows)]
    tracing::debug!("Cache Path: {}", fs::get_cache_path().display());
    tracing::debug!("Cmdline args: {:?}", cmd);
    tracing::info!("Visit https://book.veloren.net/ for an FAQ and Troubleshooting");

    if cmd.force_reset {
        std::fs::remove_dir_all(BASE_PATH.as_path())?;
    }

    // GUI
    if cmd.action.is_none() {
        match gui::run(cmd.clone()) {
            Ok(_) => return Ok(()),
            Err(_) => {
                tracing::error!("Failed to start GUI. Falling back to terminal...");
                cmd.action = Some(Action::Run);
            },
        }
    }

    // CLI
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()?;

    // let the user know incase airshipper can be updated.
    #[cfg(windows)]
    if let Ok(Some(release)) = crate::windows::query() {
        tracing::info!(
            "New Airshipper release found: {}. Run `airshipper upgrade` to update.",
            release.version
        );
    }

    rt.block_on(async {
        let mut profile = Profile::load();

        // handle arguments
        process_arguments(&mut profile, cmd.action.unwrap(), cmd.verbose).await?;

        // Save state
        profile.save_ref().await?;

        Ok::<(), ClientError>(())
    })
}

async fn process_arguments(
    profile: &mut Profile,
    action: Action,
    verbose: u8,
) -> Result<()> {
    profile.log_level = match verbose {
        0 => LogLevel::Default,
        1 => LogLevel::Debug,
        _ => LogLevel::Trace,
    };

    match action {
        Action::Update => update(profile, true).await?,
        Action::Start => start(profile, None).await?,
        Action::Run => {
            if let Err(e) = update(profile, false).await {
                tracing::error!(
                    ?e,
                    "Couldn't update the game, starting installed version."
                );
            }
            start(profile, None).await?
        },
        Action::Config => config(profile).await?,
        #[cfg(windows)]
        Action::Upgrade => {
            tokio::task::block_in_place(upgrade)?;
        },
    }
    Ok(())
}

async fn update(profile: &mut Profile, do_not_ask: bool) -> Result<()> {
    use crate::update::{Progress, update};
    use indicatif::{ProgressBar, ProgressStyle};

    let progress_bar = ProgressBar::new(100).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.green/white}] {msg} [{eta}]")
            .unwrap()
            .progress_chars("=>-"),
    );
    progress_bar.set_message("Evaluating Update");

    tracing::debug!("start updating");

    let mut stream = update(profile.clone()).boxed();

    while let Some(progress) = stream.next().await {
        match progress {
            Progress::ReadyToSync { version } => {
                tracing::debug!(?version);

                if !do_not_ask {
                    tracing::info!("Update found, do you want to update? [Y/n]");
                    if !confirm_action()? {
                        // No update for you :/
                        tracing::info!("skipping update.");
                        break;
                    }
                }
            },
            Progress::Incomplete {
                download,
                unzip,
                delete,
            } => {
                let (step, progress) = match (
                    download.is_finished(),
                    unzip.is_finished(),
                    delete.is_finished(),
                ) {
                    (false, _, _) => ("Downloading", &download),
                    (true, false, _) => ("Unzipping", &unzip),
                    (true, true, false) => ("Deleting", &delete),
                    (true, true, true) => ("Finalizing", &unzip),
                };
                progress_bar.set_position(progress.percent_complete());
                progress_bar.set_message(format!(
                    "{} / {} ({step})",
                    pretty_bytes(progress.processed_bytes()),
                    pretty_bytes(progress.total_bytes()),
                ));
            },
            Progress::Successful(new_profile) => {
                tracing::debug!("Updating profile");
                *profile = new_profile;
                // Save state
                profile.save_ref().await?;
                return Ok(());
            },
            Progress::Errored(e) => {
                return Err(e);
            },
            Progress::Offline => {
                return Err(ClientError::Custom("No internet connection".to_string()));
            },
        }
    }
    Ok(())
}

async fn start(profile: &Profile, game_server_address: Option<String>) -> Result<()> {
    if !profile.installed() {
        tracing::info!("Profile is not installed. Install it via `airshipper update`");
        return Ok(());
    }

    tracing::info!("Starting...");
    let mut stream = crate::io::stream_process(&mut Profile::start(
        profile,
        game_server_address.as_deref(),
    ))?
    .boxed();

    while let Some(progress) = stream.next().await {
        match progress {
            io::ProcessUpdate::Line(line) => tracing::info!("[Veloren] {}", line),
            io::ProcessUpdate::Exit(exit) => {
                tracing::info!("Veloren exited with {}", exit)
            },
            io::ProcessUpdate::Error(e) => return Err(e.into()),
        }
    }
    Ok(())
}

async fn config(profile: &mut Profile) -> Result<()> {
    use colored::Colorize;

    let mut editor = rustyline::DefaultEditor::new()?;

    'main: loop {
        println!("===== Current configuration =====");
        let options = [
            ("Environment variables", profile.env_vars.to_string()),
            ("Graphics backend", profile.wgpu_backend.to_string()),
        ];
        for (idx, (k, v)) in options.iter().enumerate() {
            println!("- ({}) {k} = {v}", (idx + 1).to_string().blue());
        }
        println!("Which setting do you want to change? (use 'q' to quit)");

        loop {
            match editor
                .readline(&format!("{} > ", format!("1-{}", options.len()).blue()))?
                .trim()
            {
                "1" => {
                    println!(
                        "What should the environment variables be? (use 'q' to quit)"
                    );
                    println!(
                        "{}",
                        "Hint: Environment variables should be defined as key-value \
                         pairs, separated by commands.\nExample: FOO=BAR,BAZ=BIZ"
                            .dimmed()
                    );
                    loop {
                        let input = editor
                            .readline_with_initial("> ", (&profile.env_vars, ""))?;
                        if input.trim() == "q" {
                            break;
                        } else {
                            let (_, errs) = parse_env_vars(&input);
                            if !errs.is_empty() {
                                println!(
                                    "{}: Invalid environment variables:",
                                    "ERROR".red()
                                );
                                for e in errs {
                                    println!("- {e}");
                                }
                            } else {
                                profile.env_vars = input.clone();
                                println!(
                                    "{}: Environment variables have been set to \
                                     '{input}'.",
                                    "OK".green()
                                );
                                continue 'main;
                            }
                        }
                    }
                },
                "2" => {
                    println!(
                        "Which graphics backend do you want to use? (use 'q' to quit)"
                    );
                    for (idx, backend) in
                        profile.supported_wgpu_backends.iter().enumerate()
                    {
                        println!("- ({}) {}", (idx + 1).to_string().blue(), backend);
                    }
                    loop {
                        let input = editor.readline(&format!(
                            "{} > ",
                            format!("1-{}", profile.supported_wgpu_backends.len()).blue()
                        ))?;
                        if input.trim() == "q" {
                            break;
                        } else if let Some(backend) = input
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .and_then(|idx| profile.supported_wgpu_backends.get(idx))
                            .copied()
                        {
                            profile.wgpu_backend = backend;
                            println!(
                                "{}: The graphics backend has been set to '{backend}'.",
                                "OK".green()
                            );
                            continue 'main;
                        } else {
                            println!("{}: Invalid option '{input}'", "ERROR".red());
                        }
                    }
                },
                "q" => break 'main Ok(()),
                input => println!("{}: Invalid option '{input}'.", "ERROR".red()),
            }
        }
    }
}

#[cfg(windows)]
fn upgrade() -> Result<()> {
    match crate::windows::query()? {
        Some(release) => {
            tracing::info!("Found new Airshipper release: {}", release.version);
            crate::windows::update(&release)?;
        },
        None => tracing::info!("Airshipper is up-to-date."),
    }
    Ok(())
}

/// Will read from stdin for confirmation
/// NOTE: no input = true
/// Temporary...
pub fn confirm_action() -> Result<bool> {
    let mut buffer = String::new();
    let _ = std::io::stdin().read_line(&mut buffer)?;
    buffer = buffer.to_lowercase();

    if buffer.trim().is_empty() || buffer.starts_with('y') {
        return Ok(true);
    } else if buffer.starts_with('n') {
        return Ok(false);
    }
    Ok(false)
}
