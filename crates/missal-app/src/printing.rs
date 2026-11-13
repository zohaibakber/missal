use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use gpui_kit::App;

const SPOOL_DIRECTORY: &str = "missal-print";
const SPOOL_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogViewer {
    pub program: PathBuf,
    pub arguments: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    ViewerPrintDialog(DialogViewer),
    DefaultViewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    PrintDialog,
    DefaultViewer,
}

impl Handoff {
    pub fn message(self) -> &'static str {
        match self {
            Handoff::PrintDialog => "The print dialog is open in your PDF viewer",
            Handoff::DefaultViewer => "Opened in your PDF viewer. Press Ctrl+P there to choose a printer and print",
        }
    }
}

pub struct Environment<'a> {
    pub windows: bool,
    pub variable: &'a dyn Fn(&str) -> Option<PathBuf>,
    pub exists: &'a dyn Fn(&Path) -> bool,
    pub executable_directory: Option<PathBuf>,
    pub search_path: Vec<PathBuf>,
}

impl Environment<'_> {
    fn find_on_path(&self, names: &[&str]) -> Option<PathBuf> {
        self.search_path
            .iter()
            .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
            .find(|candidate| (self.exists)(candidate))
    }
}

pub fn choose_route(environment: &Environment<'_>) -> Route {
    if environment.windows {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(directory) = &environment.executable_directory {
            candidates.push(directory.join("SumatraPDF.exe"));
            candidates.push(directory.join("resources").join("SumatraPDF.exe"));
        }
        for (variable, suffix) in [
            ("LOCALAPPDATA", "SumatraPDF"),
            ("ProgramFiles", "SumatraPDF"),
            ("ProgramFiles(x86)", "SumatraPDF"),
        ] {
            if let Some(root) = (environment.variable)(variable) {
                candidates.push(root.join(suffix).join("SumatraPDF.exe"));
            }
        }
        let found = candidates
            .into_iter()
            .find(|candidate| (environment.exists)(candidate))
            .or_else(|| environment.find_on_path(&["SumatraPDF.exe"]));
        return match found {
            Some(program) => Route::ViewerPrintDialog(DialogViewer {
                program,
                arguments: vec!["-print-dialog", "-exit-when-done"],
            }),
            None => Route::DefaultViewer,
        };
    }
    match environment.find_on_path(&["okular"]) {
        Some(program) => Route::ViewerPrintDialog(DialogViewer {
            program,
            arguments: vec!["--print"],
        }),
        None => Route::DefaultViewer,
    }
}

pub fn current_route() -> Route {
    let variable = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let exists = |path: &Path| path.is_file();
    let search_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    let executable_directory = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    choose_route(&Environment {
        windows: cfg!(target_os = "windows"),
        variable: &variable,
        exists: &exists,
        executable_directory,
        search_path,
    })
}

pub fn file_name_for(title: &str) -> String {
    let mut name = String::with_capacity(title.len());
    for c in title.trim().chars() {
        let replacement = match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        };
        name.push(replacement);
    }
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.trim_matches(|c: char| c == '.' || c == ' ' || c == '-').to_owned();
    let name = if name.is_empty() { "Missal".to_owned() } else { name };
    format!("{name}.pdf")
}

pub fn spool_directory() -> PathBuf {
    std::env::temp_dir().join(SPOOL_DIRECTORY)
}

pub fn spool(directory: &Path, title: &str, bytes: &[u8], now: SystemTime) -> io::Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    remove_stale(directory, now);
    let stamp = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let name = file_name_for(title);
    let stem = name.strip_suffix(".pdf").unwrap_or(&name);
    let path = directory.join(format!("{stem} {stamp}.pdf"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn remove_stale(directory: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let stale = path.extension().is_some_and(|extension| extension == "pdf")
            && entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > SPOOL_LIFETIME);
        if stale {
            std::fs::remove_file(path).ok();
        }
    }
}

pub fn hand_off(path: &Path, route: &Route, cx: &mut App) -> Handoff {
    if let Route::ViewerPrintDialog(viewer) = route {
        let launched = Command::new(&viewer.program)
            .args(&viewer.arguments)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = launched {
            std::thread::spawn(move || {
                child.wait().ok();
            });
            return Handoff::PrintDialog;
        }
    }
    cx.open_with_system(path);
    Handoff::DefaultViewer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment<'a>(
        windows: bool,
        variables: &'a dyn Fn(&str) -> Option<PathBuf>,
        exists: &'a dyn Fn(&Path) -> bool,
    ) -> Environment<'a> {
        Environment {
            windows,
            variable: variables,
            exists,
            executable_directory: Some(PathBuf::from("/app")),
            search_path: vec![PathBuf::from("/usr/bin"), PathBuf::from("/opt/tools")],
        }
    }

    #[test]
    fn file_names_drop_characters_windows_rejects() {
        assert_eq!(file_name_for("FIR 12/2019"), "FIR 12-2019.pdf");
        assert_eq!(file_name_for("  a:b*c?  "), "a-b-c.pdf");
        assert_eq!(file_name_for("ایف آئی آر ۱۲/۲۰۱۹"), "ایف آئی آر ۱۲-۲۰۱۹.pdf");
        assert_eq!(file_name_for(""), "Missal.pdf");
        assert_eq!(file_name_for("..."), "Missal.pdf");
    }

    #[test]
    fn windows_prefers_a_bundled_sumatra_with_a_print_dialog() {
        let variables = |_: &str| None;
        let exists = |path: &Path| path == Path::new("/app/SumatraPDF.exe");
        let route = choose_route(&environment(true, &variables, &exists));
        assert_eq!(
            route,
            Route::ViewerPrintDialog(DialogViewer {
                program: PathBuf::from("/app/SumatraPDF.exe"),
                arguments: vec!["-print-dialog", "-exit-when-done"],
            })
        );
    }

    #[test]
    fn windows_checks_the_install_folders_then_the_search_path() {
        let variables = |name: &str| (name == "ProgramFiles").then(|| PathBuf::from("/pf"));
        let exists = |path: &Path| path == Path::new("/pf/SumatraPDF/SumatraPDF.exe");
        assert!(matches!(
            choose_route(&environment(true, &variables, &exists)),
            Route::ViewerPrintDialog(viewer) if viewer.program == Path::new("/pf/SumatraPDF/SumatraPDF.exe")
        ));
        let on_path = |path: &Path| path == Path::new("/opt/tools/SumatraPDF.exe");
        assert!(matches!(
            choose_route(&environment(true, &|_| None, &on_path)),
            Route::ViewerPrintDialog(viewer) if viewer.program == Path::new("/opt/tools/SumatraPDF.exe")
        ));
    }

    #[test]
    fn without_a_known_viewer_the_default_viewer_is_used() {
        let none = |_: &Path| false;
        assert_eq!(choose_route(&environment(true, &|_| None, &none)), Route::DefaultViewer);
        assert_eq!(choose_route(&environment(false, &|_| None, &none)), Route::DefaultViewer);
    }

    #[test]
    fn linux_uses_okular_when_installed() {
        let exists = |path: &Path| path == Path::new("/usr/bin/okular");
        assert_eq!(
            choose_route(&environment(false, &|_| None, &exists)),
            Route::ViewerPrintDialog(DialogViewer {
                program: PathBuf::from("/usr/bin/okular"),
                arguments: vec!["--print"],
            })
        );
    }

    #[test]
    fn spooling_writes_the_pdf_and_removes_day_old_files() {
        let directory = tempfile::tempdir().unwrap();
        let stale = directory.path().join("old.pdf");
        std::fs::write(&stale, b"old").unwrap();
        let later = SystemTime::now() + SPOOL_LIFETIME + Duration::from_secs(60);
        let path = spool(directory.path(), "FIR 1/2026", b"%PDF-1.7", later).unwrap();
        assert!(!stale.exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"%PDF-1.7");
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("FIR 1-2026 "));
    }
}
