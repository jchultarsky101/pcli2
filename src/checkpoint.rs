//! Resumable folder match runs.
//!
//! A folder match over a large tenant runs for hours and holds every result in
//! memory until the report is written at the end. A dropped connection, an
//! expired credential or a closed laptop lid an hour in used to mean starting
//! over. With `--checkpoint FILE` each asset's completed search is appended to
//! the file the moment it finishes, and a re-run of the same command with the
//! same file reuses those results and searches only the assets that are left.
//!
//! # File format
//!
//! JSON Lines. The first line is a header carrying the run's [`Fingerprint`];
//! every following line is one [`Record`]. Appending one line per completed
//! search, flushed immediately, is what makes the file safe against the process
//! dying at any moment: at worst the final line is cut short, and a cut-short
//! final line is skipped on load. A malformed line anywhere else means the file
//! was not written by this code and is refused.
//!
//! The fingerprint exists because recorded results are only valid for the run
//! that produced them. A geometric search at threshold 80 does not answer a
//! geometric search at threshold 90, and neither answers a part search. Reusing
//! a file across different runs is refused rather than silently producing a
//! report that mixes them.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

/// Format version written in the header. Bump when the record layout changes so
/// a file from an older build is refused instead of misread.
const FORMAT_VERSION: u32 = 1;

/// Everything that determines what a recorded search result means.
///
/// Two runs with equal fingerprints would issue the same searches and filter
/// them the same way, so one may reuse the other's results. Folder paths are
/// normalised and sorted so `-p /A -p /B` and `-p /B -p /A` are the same run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Which search: `geometric`, `part` or `visual`.
    pub search: String,
    pub tenant: Uuid,
    pub folders: Vec<String>,
    pub recursive: bool,
    pub exclusive: bool,
    /// The threshold sent to the server, as its exact bit pattern: a float
    /// compared through its `Display` form would treat 80 and 80.0000001 alike.
    pub threshold_bits: u64,
    /// Visual search's `--limit`; `None` for the other searches.
    pub limit: Option<usize>,
}

impl Fingerprint {
    pub fn new(
        search: &str,
        tenant: Uuid,
        folders: &[String],
        recursive: bool,
        exclusive: bool,
        threshold: f64,
        limit: Option<usize>,
    ) -> Self {
        let mut folders: Vec<String> = folders.iter().map(crate::model::normalize_path).collect();
        folders.sort();
        folders.dedup();
        Self {
            search: search.to_string(),
            tenant,
            folders,
            recursive,
            exclusive,
            threshold_bits: threshold.to_bits(),
            limit,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Header {
    pcli2_checkpoint: u32,
    fingerprint: Fingerprint,
}

#[derive(Debug, Deserialize)]
struct Record<T> {
    asset: Uuid,
    matches: Vec<T>,
}

/// The borrowed form of [`Record`], for writing without cloning the matches.
#[derive(Debug, Serialize)]
struct RecordRef<'a, T> {
    asset: Uuid,
    matches: &'a [T],
}

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("cannot use checkpoint file '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file exists but was written by a different command line: another
    /// search type, tenant, folder set, threshold or limit.
    #[error(
        "checkpoint file '{path}' belongs to a different run ({recorded}); delete it or choose another --checkpoint path"
    )]
    Mismatch { path: PathBuf, recorded: String },
    #[error("checkpoint file '{path}' is not a pcli2 checkpoint (line {line}: {reason})")]
    Malformed {
        path: PathBuf,
        line: usize,
        reason: String,
    },
}

/// A checkpoint file open for appending, plus what was already in it.
#[derive(Debug)]
pub struct Checkpoint<T> {
    path: PathBuf,
    writer: Mutex<File>,
    reused: usize,
    recorded: AtomicUsize,
    write_failed: AtomicBool,
    _record: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> Checkpoint<T> {
    /// Open `path` for this run, creating it if it does not exist.
    ///
    /// Returns the checkpoint and the results already recorded in it, keyed by
    /// asset. An existing file whose header does not match `fingerprint` is
    /// refused.
    pub fn open(
        path: &Path,
        fingerprint: Fingerprint,
    ) -> Result<(Self, HashMap<Uuid, Vec<T>>), CheckpointError> {
        let io = |source: std::io::Error| CheckpointError::Io {
            path: path.to_path_buf(),
            source,
        };

        let existing = match std::fs::metadata(path) {
            Ok(meta) => meta.len() > 0,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(io(e)),
        };

        let done = if existing {
            Self::load(path, &fingerprint)?
        } else {
            HashMap::new()
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(io)?;
        if !existing {
            let header = Header {
                pcli2_checkpoint: FORMAT_VERSION,
                fingerprint,
            };
            let mut line = serde_json::to_string(&header).map_err(|e| io(e.into()))?;
            line.push('\n');
            file.write_all(line.as_bytes()).map_err(io)?;
            file.flush().map_err(io)?;
        }

        let reused = done.len();
        Ok((
            Self {
                path: path.to_path_buf(),
                writer: Mutex::new(file),
                reused,
                recorded: AtomicUsize::new(0),
                write_failed: AtomicBool::new(false),
                _record: PhantomData,
            },
            done,
        ))
    }

    fn load(
        path: &Path,
        fingerprint: &Fingerprint,
    ) -> Result<HashMap<Uuid, Vec<T>>, CheckpointError> {
        let io = |source: std::io::Error| CheckpointError::Io {
            path: path.to_path_buf(),
            source,
        };
        let malformed = |line: usize, reason: String| CheckpointError::Malformed {
            path: path.to_path_buf(),
            line,
            reason,
        };

        let file = File::open(path).map_err(io)?;
        let mut lines = BufReader::new(file).lines();

        let header_line = lines
            .next()
            .ok_or_else(|| malformed(1, "empty file".into()))?
            .map_err(io)?;
        let header: Header = serde_json::from_str(&header_line)
            .map_err(|e| malformed(1, format!("bad header: {}", e)))?;
        if header.pcli2_checkpoint != FORMAT_VERSION {
            return Err(malformed(
                1,
                format!(
                    "format version {} is not supported (this build writes {})",
                    header.pcli2_checkpoint, FORMAT_VERSION
                ),
            ));
        }
        if &header.fingerprint != fingerprint {
            return Err(CheckpointError::Mismatch {
                path: path.to_path_buf(),
                recorded: describe(&header.fingerprint),
            });
        }

        let mut done = HashMap::new();
        let mut pending: Option<(usize, String)> = None;
        for (index, line) in lines.enumerate() {
            let line = line.map_err(io)?;
            // Parse lazily by one line so a cut-short final line can be told
            // apart from corruption in the middle of the file.
            if let Some((number, text)) = pending.take() {
                let record: Record<T> =
                    serde_json::from_str(&text).map_err(|e| malformed(number, e.to_string()))?;
                done.insert(record.asset, record.matches);
            }
            pending = Some((index + 2, line));
        }
        if let Some((number, text)) = pending {
            match serde_json::from_str::<Record<T>>(&text) {
                Ok(record) => {
                    done.insert(record.asset, record.matches);
                }
                Err(e) => {
                    tracing::debug!(
                        "Ignoring incomplete final line {} of checkpoint '{}': {}",
                        number,
                        path.display(),
                        e
                    );
                }
            }
        }
        Ok(done)
    }

    /// Record a completed search for `asset`.
    ///
    /// A write failure does not fail the search: the result is still reported,
    /// it just cannot be resumed from. The first failure is surfaced once; the
    /// rest are logged.
    pub fn record(&self, asset: Uuid, matches: &[T]) {
        let record = RecordRef { asset, matches };
        let mut line = match serde_json::to_string(&record) {
            Ok(line) => line,
            Err(e) => {
                self.report_write_failure(&e.to_string());
                return;
            }
        };
        line.push('\n');
        let result = {
            let mut file = match self.writer.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            file.write_all(line.as_bytes()).and_then(|_| file.flush())
        };
        match result {
            Ok(()) => {
                self.recorded.fetch_add(1, Ordering::SeqCst);
            }
            Err(e) => self.report_write_failure(&e.to_string()),
        }
    }

    fn report_write_failure(&self, reason: &str) {
        if !self.write_failed.swap(true, Ordering::SeqCst) {
            crate::error_utils::report_warning(&format!(
                "Could not write to checkpoint file '{}': {}. The run continues, but it cannot be resumed from this point.",
                self.path.display(),
                reason
            ));
        } else {
            tracing::debug!(
                "Checkpoint write to '{}' failed again: {}",
                self.path.display(),
                reason
            );
        }
    }

    /// Results reused from a previous run.
    pub fn reused(&self) -> usize {
        self.reused
    }

    /// Results written by this run.
    pub fn recorded(&self) -> usize {
        self.recorded.load(Ordering::SeqCst)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Remove the file: the run completed and there is nothing to resume.
    pub fn finish(&self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::debug!(
                "Could not remove checkpoint file '{}': {}",
                self.path.display(),
                e
            );
        }
    }
}

fn describe(fingerprint: &Fingerprint) -> String {
    let mut parts = vec![
        format!("{} search", fingerprint.search),
        format!("tenant {}", fingerprint.tenant),
        format!("folders {}", fingerprint.folders.join(", ")),
        format!("threshold {}", f64::from_bits(fingerprint.threshold_bits)),
    ];
    if fingerprint.recursive {
        parts.push("recursive".into());
    }
    if fingerprint.exclusive {
        parts.push("exclusive".into());
    }
    if let Some(limit) = fingerprint.limit {
        parts.push(format!("limit {}", limit));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> Fingerprint {
        Fingerprint::new(
            "geometric",
            Uuid::from_u128(1),
            &["/B".to_string(), "/A/".to_string()],
            true,
            false,
            80.0,
            None,
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pcli2-checkpoint-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn folder_order_and_trailing_slashes_do_not_change_the_fingerprint() {
        let a = Fingerprint::new(
            "part",
            Uuid::from_u128(1),
            &["/B".into(), "/A/".into()],
            false,
            false,
            0.0,
            None,
        );
        let b = Fingerprint::new(
            "part",
            Uuid::from_u128(1),
            &["/A".into(), "/B".into()],
            false,
            false,
            0.0,
            None,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn records_survive_reopening_and_the_file_is_removed_on_finish() {
        let path = scratch("roundtrip.jsonl");
        let _ = std::fs::remove_file(&path);

        let (checkpoint, done) = Checkpoint::<String>::open(&path, fingerprint()).unwrap();
        assert!(done.is_empty());
        assert_eq!(checkpoint.reused(), 0);
        checkpoint.record(Uuid::from_u128(10), &["x".to_string(), "y".to_string()]);
        checkpoint.record(Uuid::from_u128(11), &[]);
        assert_eq!(checkpoint.recorded(), 2);
        drop(checkpoint);

        let (checkpoint, done) = Checkpoint::<String>::open(&path, fingerprint()).unwrap();
        assert_eq!(checkpoint.reused(), 2);
        assert_eq!(done[&Uuid::from_u128(10)], vec!["x", "y"]);
        assert!(done[&Uuid::from_u128(11)].is_empty());

        checkpoint.finish();
        assert!(!path.exists());
    }

    #[test]
    fn a_cut_short_final_line_is_skipped_but_earlier_corruption_is_refused() {
        let path = scratch("truncated.jsonl");
        let _ = std::fs::remove_file(&path);
        let (checkpoint, _) = Checkpoint::<String>::open(&path, fingerprint()).unwrap();
        checkpoint.record(Uuid::from_u128(1), &["ok".to_string()]);
        drop(checkpoint);

        // Simulate the process dying mid-write.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"asset\":\"00000000-0000-0000-0000-0000000000")
            .unwrap();
        drop(file);

        let (_, done) = Checkpoint::<String>::open(&path, fingerprint()).unwrap();
        assert_eq!(done.len(), 1);

        // Garbage followed by a good line is not a crash artefact.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"\n{\"asset\":\"00000000-0000-0000-0000-000000000002\",\"matches\":[]}\n")
            .unwrap();
        drop(file);
        let err = Checkpoint::<String>::open(&path, fingerprint()).unwrap_err();
        assert!(matches!(err, CheckpointError::Malformed { .. }), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_from_a_different_run_is_refused() {
        let path = scratch("mismatch.jsonl");
        let _ = std::fs::remove_file(&path);
        let (checkpoint, _) = Checkpoint::<String>::open(&path, fingerprint()).unwrap();
        drop(checkpoint);

        let mut other = fingerprint();
        other.threshold_bits = 90.0f64.to_bits();
        let err = Checkpoint::<String>::open(&path, other).unwrap_err();
        match err {
            CheckpointError::Mismatch { recorded, .. } => {
                assert!(recorded.contains("geometric search"), "{recorded}");
                assert!(recorded.contains("threshold 80"), "{recorded}");
                assert!(recorded.contains("recursive"), "{recorded}");
            }
            other => panic!("expected Mismatch, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_that_is_not_a_checkpoint_is_refused() {
        let path = scratch("notours.jsonl");
        std::fs::write(&path, "NAME,PATH\nfoo,/foo\n").unwrap();
        let err = Checkpoint::<String>::open(&path, fingerprint()).unwrap_err();
        assert!(
            matches!(err, CheckpointError::Malformed { line: 1, .. }),
            "{err}"
        );
        let _ = std::fs::remove_file(&path);
    }
}
