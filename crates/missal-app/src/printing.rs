use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
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
            Handoff::DefaultViewer => {
                "Opened in your PDF viewer. Press Ctrl+P there to choose a printer and print"
            }
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
    let variable = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
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
    let name = name
        .trim_matches(|c: char| c == '.' || c == ' ' || c == '-')
        .to_owned();
    let name = if name.is_empty() {
        "Missal".to_owned()
    } else {
        name
    };
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

pub fn launch_viewer(path: &Path) -> Option<Handoff> {
    static ROUTE: OnceLock<Route> = OnceLock::new();
    let Route::ViewerPrintDialog(viewer) = ROUTE.get_or_init(current_route) else {
        return None;
    };
    let mut child = Command::new(&viewer.program)
        .args(&viewer.arguments)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    std::thread::spawn(move || {
        child.wait().ok();
    });
    Some(Handoff::PrintDialog)
}

pub fn open_default(path: &Path, cx: &mut App) -> Handoff {
    cx.open_with_system(path);
    Handoff::DefaultViewer
}
