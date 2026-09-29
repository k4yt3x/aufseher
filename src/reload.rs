//! Reloading the pattern file when it changes.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::rules::Rules;

/// How long the pattern file must go unchanged before a reload, so that a save made in several
/// steps reloads once, after the last.
const SETTLE_TIME: Duration = Duration::from_millis(500);

/// Loads the pattern file at `path` and reloads it on a thread of its own each time it changes,
/// returning the receiving end of the current rules. An edit that fails to load is logged, and the
/// current rules stay.
///
/// Two watches cover the ways a file changes. The one on the parent directory sees a new file
/// renamed over the old one, which is how many editors and deployment tools save. The one on the
/// file's inode sees writes the directory never hears of, such as through a single-file bind mount,
/// and moves to the new inode before each reload.
pub(crate) fn spawn(path: PathBuf) -> Result<watch::Receiver<Arc<Rules>>> {
    let file_name = path
        .file_name()
        .with_context(|| format!("{} does not name a file", path.display()))?
        .to_owned();
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_owned(),
        _ => PathBuf::from("."),
    };

    let (changes, changed) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        if is_change(&event, &file_name) && changes.send(()).is_err() {
            warn!("The pattern file changed, but the reloader has stopped");
        }
    })
    .context("failed to create a file watcher")?;

    // The directory watch goes first, so that an edit landing during the load still triggers a
    // reload, and the load goes before the file watch, so that a missing file reports as one.
    watch_path(&mut watcher, &directory)?;
    let rules = Rules::open(&path)?;
    info!("Loaded {rules} from {}", path.display());
    watch_path(&mut watcher, &path)?;
    info!("Watching {} for changes", path.display());

    let (sender, receiver) = watch::channel(Arc::new(rules));
    thread::Builder::new()
        .name("reloader".to_owned())
        .spawn(move || reload_on_change(watcher, &path, &changed, &sender))
        .context("failed to start the reloader")?;
    Ok(receiver)
}

/// Adds a non-recursive watch on `path`.
fn watch_path(watcher: &mut RecommendedWatcher, path: &Path) -> Result<()> {
    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", path.display()))
}

/// Returns whether `event` may have changed the file named `file_name`. Opening or reading the file
/// does not count, since every reload does both.
fn is_change(event: &notify::Result<Event>, file_name: &OsStr) -> bool {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            warn!("Error watching the pattern file: {error}");
            return false;
        }
    };

    // An overflowed event queue names no file, so any file may have changed.
    if event.need_rescan() {
        return true;
    }
    // Every other kind counts, removal included. When the old inode behind a replaced file finally
    // goes, notify drops the watch on the new one too, as it tracks watches by path; the reload the
    // removal triggers is what restores it.
    let may_modify = match event.kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    };
    may_modify
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(file_name))
}

/// Reloads the rules once each burst of changes settles, for as long as the watcher reports them.
fn reload_on_change(
    mut watcher: RecommendedWatcher,
    path: &Path,
    changed: &mpsc::Receiver<()>,
    rules: &watch::Sender<Arc<Rules>>,
) {
    while changed.recv().is_ok() {
        while changed.recv_timeout(SETTLE_TIME).is_ok() {}

        // A file replaced by a rename is a new inode. Watching it before reading it means a write
        // that lands after the read still triggers another reload.
        match watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(()) => {}
            // The reload below fails on a missing file too, and says so.
            Err(error) if matches!(error.kind, notify::ErrorKind::PathNotFound) => {}
            Err(error) => warn!(
                "Failed to watch {}, so writes the directory does not see go unnoticed: {error}",
                path.display()
            ),
        }

        match Rules::open(path) {
            Ok(reloaded) => {
                info!("Reloaded {reloaded} from {}", path.display());
                rules.send_replace(Arc::new(reloaded));
            }
            Err(error) => error!("Keeping the current patterns: {error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use tempfile::TempDir;
    use tokio::{sync::watch, time};

    use super::{SETTLE_TIME, spawn};
    use crate::rules::Rules;

    /// How long a test waits for a reload it expects.
    const RELOAD_TIMEOUT: Duration = Duration::from_secs(5);

    /// How long a test waits to be sure no reload is coming.
    const QUIET_TIME: Duration = SETTLE_TIME.saturating_mul(3);

    fn pattern_file(message_pattern: &str) -> String {
        format!("{{name_regexes: [], message_regexes: ['{message_pattern}']}}")
    }

    /// Writes a pattern file with one message pattern into a new directory and watches it.
    fn watched(message_pattern: &str) -> (TempDir, PathBuf, watch::Receiver<Arc<Rules>>) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("aufseher.yaml");
        fs::write(&path, pattern_file(message_pattern)).unwrap();
        let receiver = spawn(path.clone()).unwrap();
        (directory, path, receiver)
    }

    /// Returns whether the published rules come to match `text` within [`RELOAD_TIMEOUT`].
    async fn loads_eventually(receiver: &mut watch::Receiver<Arc<Rules>>, text: &str) -> bool {
        let wait = async {
            while receiver.borrow_and_update().match_message(text).is_none() {
                receiver.changed().await.unwrap();
            }
        };
        time::timeout(RELOAD_TIMEOUT, wait).await.is_ok()
    }

    /// Returns whether a reload arrives within [`QUIET_TIME`].
    async fn reloads_while_waiting(receiver: &watch::Receiver<Arc<Rules>>) -> bool {
        time::sleep(QUIET_TIME).await;
        receiver.has_changed().unwrap()
    }

    #[tokio::test]
    async fn an_edit_through_another_link_reloads_once() {
        let (_directory, path, mut receiver) = watched("first");

        // Like a single-file bind mount, a hard link elsewhere edits the file without the watched
        // directory hearing of it.
        let elsewhere = tempfile::tempdir().unwrap();
        let link = elsewhere.path().join("linked.yaml");
        fs::hard_link(&path, &link).unwrap();
        fs::write(&link, pattern_file("second")).unwrap();
        assert!(loads_eventually(&mut receiver, "second").await);

        // The reload reads the file, which must not count as another change.
        assert!(!reloads_while_waiting(&receiver).await);
    }

    #[tokio::test]
    async fn the_inode_watch_follows_a_replaced_file_even_if_the_old_one_closes_late() {
        let (directory, path, mut receiver) = watched("first");

        // An open handle keeps the old inode alive past the rename, so it goes only once dropped.
        let old_file = fs::File::open(&path).unwrap();
        let staged = directory.path().join("aufseher.yaml.new");
        fs::write(&staged, pattern_file("second")).unwrap();
        fs::rename(&staged, &path).unwrap();
        assert!(loads_eventually(&mut receiver, "second").await);
        drop(old_file);
        time::sleep(SETTLE_TIME).await;

        let elsewhere = tempfile::tempdir().unwrap();
        let link = elsewhere.path().join("linked.yaml");
        fs::hard_link(&path, &link).unwrap();
        fs::write(&link, pattern_file("third")).unwrap();
        assert!(loads_eventually(&mut receiver, "third").await);
    }

    #[tokio::test]
    async fn an_invalid_edit_keeps_the_current_patterns() {
        let (_directory, path, receiver) = watched("first");
        fs::write(&path, pattern_file("(unclosed")).unwrap();
        assert!(!reloads_while_waiting(&receiver).await);
        assert!(receiver.borrow().match_message("first").is_some());
    }
}
