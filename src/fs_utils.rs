//! Crash- and concurrency-safe file writes.
//!
//! pcli2 keeps its state in small files: `config.yml`, the credentials file and the
//! caches. They are rewritten in place by whichever invocation happens to run, and
//! users do run several at once (`xargs -P`, cron, CI matrices). A plain
//! `fs::write` truncates the file first, so a second process reading at that moment
//! sees an empty or half-written file, and a crash leaves it that way for good.
//!
//! Everything here writes to a temporary file in the same directory and renames it
//! over the target, which is atomic on every platform pcli2 ships for: a reader sees
//! either the old contents or the new ones, never a mix.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

/// A temporary path next to `path`, unique to this process and this call.
///
/// Two tasks in one process writing the same file must not share a temporary file,
/// or one would rename the other's half-written data into place.
fn temporary_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(
        ".{}.tmp-{}-{}",
        file_name,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_via_temporary(path: &Path, data: &[u8], private: bool) -> std::io::Result<()> {
    let tmp = temporary_path(path);
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if private {
            // Created owner-only, so the secret is never readable by others, not even
            // for the moment between creating the file and tightening its mode.
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut file = options.open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        rename_with_retry(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Whether an I/O error is one Windows reports for a file in a passing state: being
/// deleted (a lock file another process just released), held open for a moment
/// by a virus scanner or the search indexer, or briefly absent while another
/// rename replaces it. These clear within milliseconds. Elsewhere they are real
/// errors and are never retried.
fn transient_on_windows(error: &std::io::Error) -> bool {
    cfg!(windows)
        && matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
        )
}

/// `fs::rename`, retried for about a second while Windows reports the file as
/// transiently unavailable (see [`transient_on_windows`]).
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempts = 0;
    loop {
        match fs::rename(from, to) {
            Err(e) if transient_on_windows(&e) && attempts < 50 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            result => return result,
        }
    }
}

/// Replace `path` with `data` atomically.
pub fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    write_via_temporary(path, data, false)
}

/// Replace `path` with `data` atomically, readable by the owner only (`0600` on Unix).
///
/// For files holding secrets. Windows has no mode bits; the file inherits the
/// directory's ACLs there, as before.
pub fn write_private_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    write_via_temporary(path, data, true)
}

/// How long [`FileLock::acquire`] waits for another process before giving up.
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// A lock file older than this is taken to belong to a process that died holding it.
///
/// Holders only keep the lock for one read-modify-write of a small file, which takes
/// milliseconds, so anything this old is not going to be released.
const LOCK_STALE_AFTER: Duration = Duration::from_secs(30);

/// An advisory lock held for a read-modify-write of one file.
///
/// Atomic writes stop a reader from seeing a torn file, but two processes that each
/// read, change one entry and write back still lose one of the changes. Holding this
/// lock across the whole cycle serialises them. The lock is a `<file>.lock` sibling
/// created exclusively, which works the same on every platform and needs no extra
/// dependency; it is removed when the guard is dropped.
pub struct FileLock {
    path: PathBuf,
}

impl FileLock {
    /// Take the lock for `target`, waiting up to ten seconds for another holder.
    pub fn acquire(target: &Path) -> std::io::Result<Self> {
        let path = {
            let mut name = target.as_os_str().to_owned();
            name.push(".lock");
            PathBuf::from(name)
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let deadline = std::time::Instant::now() + LOCK_WAIT;
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Self { path });
                }
                // Windows answers PermissionDenied while the previous holder's lock
                // file is still being deleted: busy, like AlreadyExists.
                Err(e)
                    if e.kind() == std::io::ErrorKind::AlreadyExists
                        || (cfg!(windows) && e.kind() == std::io::ErrorKind::PermissionDenied) =>
                {
                    if Self::is_stale(&path) {
                        tracing::debug!("Removing stale lock file {}", path.display());
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            format!(
                                "'{}' is locked by another pcli2 process; if none is running, delete the lock file",
                                path.display()
                            ),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn is_stale(path: &Path) -> bool {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > LOCK_STALE_AFTER)
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write_atomically(&path, b"one").unwrap();
        write_atomically(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 1, "only the target file remains");
    }

    #[cfg(unix)]
    #[test]
    fn private_write_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.json");
        write_private_atomically(&path, b"s3cret").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("state.json");
        let lock_path = dir.path().join("state.json.lock");
        {
            let _guard = FileLock::acquire(&target).unwrap();
            assert!(lock_path.exists());
            // Another holder cannot take it while this one lives.
            assert!(fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
                .is_err());
        }
        assert!(!lock_path.exists(), "dropping the guard releases the lock");
        FileLock::acquire(&target).unwrap();
    }

    #[test]
    fn concurrent_writers_under_the_lock_lose_no_update() {
        // The scenario that used to wipe credentials: several processes each add
        // their own entry to one shared file.
        let dir = tempfile::tempdir().unwrap();
        let target = std::sync::Arc::new(dir.path().join("shared.txt"));
        write_atomically(&target, b"").unwrap();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let target = target.clone();
                std::thread::spawn(move || {
                    let _guard = FileLock::acquire(&target).unwrap();
                    let mut contents = fs::read_to_string(&*target).unwrap();
                    contents.push_str(&format!("{}\n", i));
                    write_atomically(&target, contents.as_bytes()).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let lines = fs::read_to_string(&*target).unwrap().lines().count();
        assert_eq!(lines, 8);
    }
}
