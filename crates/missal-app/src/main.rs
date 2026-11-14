#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell, window_options};
use missal_app::paths::Paths;
use missal_app::prefs::Preferences;
use missal_app::seed::seed_firs;

struct Args {
    data_dir: Option<PathBuf>,
    seed: Option<usize>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { data_dir: None, seed: None };
    let mut iterator = std::env::args().skip(1);
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--data-dir" => {
                args.data_dir = Some(iterator.next().ok_or("--data-dir needs a directory")?.into());
            }
            "--seed" => {
                let count = iterator.next().ok_or("--seed needs a count")?;
                args.seed = Some(count.parse().map_err(|_| format!("{count} is not a number"))?);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if let Ok(directory) = std::env::var("MISSAL_DATA_DIR")
        && args.data_dir.is_none()
        && !directory.is_empty()
    {
        args.data_dir = Some(directory.into());
    }
    Ok(args)
}

fn main() -> ExitCode {
    missal_app::update::installer::run_hooks();
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("missal: {message}\nusage: missal [--data-dir DIR] [--seed COUNT]");
            return ExitCode::from(2);
        }
    };
    let paths = Paths::resolve(args.data_dir.as_deref());
    if let Err(error) = paths.ensure_directory() {
        eprintln!("missal: could not create {}: {error}", paths.directory.display());
        return ExitCode::FAILURE;
    }
    if let Some(count) = args.seed {
        if paths.is_default {
            eprintln!("missal: refusing to seed the real data directory; pass --data-dir with a scratch directory");
            return ExitCode::from(2);
        }
        return match seed_firs(&paths.database, count) {
            Ok(count) => {
                println!("seeded {count} FIRs into {}", paths.database.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("missal: seeding failed: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let check_updates = missal_app::update::controller::enabled(!paths.is_default);
    let preferences = Preferences::load(&paths.preferences);
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            let db = Db::spawn(paths.database.clone());
            let models = init_app(cx, db, Some(&paths), preferences.clone());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    let prefs = missal_app::state::Models::get(cx).prefs;
                    prefs.update(cx, |prefs, _| prefs.flush());
                    cx.quit();
                }
            })
            .detach();
            let options = window_options(&preferences, cx);
            match open_shell(&models, options, cx) {
                Ok((window, _)) => {
                    if check_updates {
                        missal_app::update::controller::start(window, cx);
                    }
                }
                Err(error) => {
                    eprintln!("missal: could not open the window: {error:#}");
                    cx.quit();
                }
            }
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
