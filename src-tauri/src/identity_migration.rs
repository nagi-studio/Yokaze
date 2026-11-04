//! Moves user data from the inherited Electron-era bundle id to the current
//! one. WebKit, WebView2 and Tauri all key their storage directories by the
//! identifier, so without this an update would open logged out with default
//! settings. Runs before any webview or plugin touches those directories.

use std::{
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const LEGACY_IDENTIFIER: &str = "com.electron.yesplaymusic";

/// Written into the new config directory once the move is settled. Until
/// then, anything under the new id came from a launch whose move failed and
/// loses to the user's real data still under the legacy id.
const MARKER: &str = "identity-migration.done";

/// An updater restart spawns the new version before the old one exits.
const LEGACY_EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// One storage location under both ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing under the legacy id, or already migrated.
    Settled,
    Migrated,
    /// The legacy app is still running; this launch must not start.
    LegacyRunning,
    /// A move failed and was rolled back; the next launch retries.
    Failed(String),
}

pub fn run(identifier: &str) -> Outcome {
    if identifier == LEGACY_IDENTIFIER {
        return Outcome::Settled;
    }
    let Some(plan) = Plan::for_platform(identifier) else {
        return Outcome::Settled;
    };
    if plan.marker.exists() || !plan.has_legacy_data() {
        return Outcome::Settled;
    }
    if !wait_for_legacy_exit(LEGACY_EXIT_TIMEOUT) {
        return Outcome::LegacyRunning;
    }
    plan.execute()
}

#[derive(Debug)]
pub struct Plan {
    /// Cookies, web storage and settings: they move together or not at all,
    /// so a failure never leaves the session split across two ids.
    pub session: Vec<Move>,
    /// Regenerable caches: best effort.
    pub caches: Vec<Move>,
    pub marker: PathBuf,
}

impl Plan {
    fn for_platform(identifier: &str) -> Option<Self> {
        let pair = |root: &Path, suffix: &str| Move {
            from: root.join(format!("{LEGACY_IDENTIFIER}{suffix}")),
            to: root.join(format!("{identifier}{suffix}")),
        };
        let config = dirs::config_dir()?;
        let marker = config.join(identifier).join(MARKER);

        #[cfg(target_os = "macos")]
        {
            let library = dirs::home_dir()?.join("Library");
            let http = library.join("HTTPStorages");
            Some(Self {
                session: vec![
                    pair(&config, ""),
                    pair(&library.join("WebKit"), ""),
                    pair(&http, ".binarycookies"),
                    pair(&http, ""),
                ],
                caches: vec![pair(&dirs::cache_dir()?, "")],
                marker,
            })
        }
        #[cfg(windows)]
        {
            // EBWebView, the WebView2 profile, lives under local data.
            Some(Self {
                session: vec![pair(&config, ""), pair(&dirs::data_local_dir()?, "")],
                caches: Vec::new(),
                marker,
            })
        }
        #[cfg(target_os = "linux")]
        {
            // WebKitGTK keeps both data and cache under local data.
            Some(Self {
                session: vec![pair(&config, ""), pair(&dirs::data_local_dir()?, "")],
                caches: vec![pair(&dirs::cache_dir()?, "")],
                marker,
            })
        }
        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        {
            let _ = (pair, marker);
            None
        }
    }

    fn has_legacy_data(&self) -> bool {
        self.session.iter().any(|item| exists(&item.from))
    }

    pub fn execute(&self) -> Outcome {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let mut done = Vec::new();
        for item in &self.session {
            if let Err(error) = move_into_place(item, stamp, &mut done) {
                roll_back(done);
                return Outcome::Failed(format!(
                    "{} -> {}: {error}",
                    item.from.display(),
                    item.to.display()
                ));
            }
        }
        for item in &self.caches {
            // A cache that cannot move is rebuilt; keep the old one for now.
            let mut cache_done = Vec::new();
            if move_into_place(item, stamp, &mut cache_done).is_err() {
                roll_back(cache_done);
            }
        }
        if let Some(parent) = self.marker.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = fs::write(&self.marker, LEGACY_IDENTIFIER) {
            // The data already moved; without a marker the next launch only
            // finds nothing left under the legacy id and settles.
            eprintln!("[identity-migration] marker not written: {error}");
        }
        Outcome::Migrated
    }
}

/// A completed rename, newest last, so a failure can undo them in reverse.
#[derive(Debug)]
struct Done {
    from: PathBuf,
    to: PathBuf,
}

fn move_into_place(item: &Move, stamp: u64, done: &mut Vec<Done>) -> io::Result<()> {
    if !exists(&item.from) {
        return Ok(());
    }
    if exists(&item.to) {
        // Left by a launch whose migration failed or was skipped; the
        // legacy copy is the user's real data. Kept aside, not deleted.
        let mut aside = item.to.clone().into_os_string();
        aside.push(format!(".pre-migration-{stamp}"));
        let aside = PathBuf::from(aside);
        fs::rename(&item.to, &aside)?;
        done.push(Done {
            from: item.to.clone(),
            to: aside,
        });
    }
    fs::rename(&item.from, &item.to)?;
    done.push(Done {
        from: item.from.clone(),
        to: item.to.clone(),
    });
    Ok(())
}

fn roll_back(done: Vec<Done>) {
    for step in done.into_iter().rev() {
        if let Err(error) = fs::rename(&step.to, &step.from) {
            eprintln!(
                "[identity-migration] could not restore {}: {error}",
                step.from.display()
            );
        }
    }
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn wait_for_legacy_exit(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut was_running = false;
    while legacy_instance_running() {
        was_running = true;
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if was_running {
        // WebKit's network process flushes cookies just after the app exits.
        thread::sleep(Duration::from_millis(500));
    }
    true
}

#[cfg(target_os = "macos")]
fn legacy_instance_running() -> bool {
    let socket = format!(
        "/tmp/{}_si.sock",
        LEGACY_IDENTIFIER.replace(['.', '-'], "_")
    );
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

/// Hands this launch to the still-running legacy app the way its
/// single-instance plugin expects, so the click still opens a window.
#[cfg(target_os = "macos")]
pub fn activate_legacy_instance() {
    use std::io::Write;

    let socket = format!(
        "/tmp/{}_si.sock",
        LEGACY_IDENTIFIER.replace(['.', '-'], "_")
    );
    if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let args = std::env::args().collect::<Vec<_>>().join("\0");
        let _ = write!(stream, "{}\0\0{args}", cwd.display());
    }
}

#[cfg(windows)]
fn legacy_instance_running() -> bool {
    use windows::{core::HSTRING, Win32::UI::WindowsAndMessaging::FindWindowW};

    let class = HSTRING::from(format!("{LEGACY_IDENTIFIER}-sic"));
    let title = HSTRING::from(format!("{LEGACY_IDENTIFIER}-siw"));
    // SAFETY: both strings outlive the call.
    unsafe { FindWindowW(&class, &title) }.is_ok_and(|window| !window.is_invalid())
}

#[cfg(target_os = "linux")]
fn legacy_instance_running() -> bool {
    let name = format!("{LEGACY_IDENTIFIER}.SingleInstance");
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(name.as_str()) else {
        return false;
    };
    proxy.name_has_owner(name).unwrap_or(false)
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn legacy_instance_running() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(root: &Path) -> Plan {
        let pair = |name: &str| Move {
            from: root.join(format!("old{name}")),
            to: root.join(format!("new{name}")),
        };
        Plan {
            session: vec![pair("-config"), pair("-webkit"), pair(".cookies")],
            caches: vec![pair("-cache")],
            marker: root.join("new-config").join(MARKER),
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn moves_every_location_and_marks_the_move_done() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        write(&root.join("old-config/window.json"), "frame");
        write(&root.join("old-webkit/Default/salt"), "salt");
        write(&root.join("old.cookies"), "login");
        write(&root.join("old-cache/blob"), "cache");

        assert_eq!(plan(root).execute(), Outcome::Migrated);

        assert_eq!(read(&root.join("new-config/window.json")), "frame");
        assert_eq!(read(&root.join("new-webkit/Default/salt")), "salt");
        assert_eq!(read(&root.join("new.cookies")), "login");
        assert_eq!(read(&root.join("new-cache/blob")), "cache");
        assert!(!root.join("old-webkit").exists());
        assert!(root.join("new-config").join(MARKER).exists());
    }

    #[test]
    fn legacy_data_wins_over_a_fresh_profile_and_the_fresh_one_is_kept_aside() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        write(&root.join("old.cookies"), "login");
        write(&root.join("new.cookies"), "logged out");

        assert_eq!(plan(root).execute(), Outcome::Migrated);

        assert_eq!(read(&root.join("new.cookies")), "login");
        let aside = fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("new.cookies.pre-migration-")
            })
            .expect("the fresh profile is kept aside");
        assert_eq!(read(&aside.path()), "logged out");
    }

    #[test]
    fn a_failed_move_restores_everything_already_moved() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        write(&root.join("old-config/window.json"), "frame");
        write(&root.join("old-webkit/Default/salt"), "salt");
        write(&root.join("old.cookies"), "login");
        // The last rename fails: its target's parent does not exist.
        let mut plan = plan(root);
        plan.session[2].to = root.join("missing/new.cookies");

        assert!(matches!(plan.execute(), Outcome::Failed(_)));

        assert_eq!(read(&root.join("old-config/window.json")), "frame");
        assert_eq!(read(&root.join("old-webkit/Default/salt")), "salt");
        assert_eq!(read(&root.join("old.cookies")), "login");
        assert!(!root.join("new-config").exists());
        assert!(!root.join("new-webkit").exists());
        assert!(!plan.marker.exists());
    }
}
