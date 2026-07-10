use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const TEMP_FILE_PREFIX: &str = ".neo-rimage-";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommitPolicy {
    FailIfExists,
    Replace { backup_path: Option<PathBuf> },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CommitOutcome {
    pub backup_path: Option<PathBuf>,
    pub cleanup_warnings: Vec<String>,
}

/// A same-directory temporary output that is removed unless it is committed.
///
/// Keeping the temporary file next to the target makes the final rename stay on
/// the same filesystem. `OutputTransaction` deliberately knows nothing about
/// Tauri, jobs, or image codecs; the engine owns the orchestration around it.
pub(crate) struct OutputTransaction {
    target_path: PathBuf,
    temp_path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl OutputTransaction {
    pub fn begin(target_path: impl Into<PathBuf>, correlation: &str) -> io::Result<Self> {
        let target_path = target_path.into();
        let parent = parent_or_current(&target_path);
        fs::create_dir_all(parent)?;

        let label = safe_file_label(correlation);
        for _ in 0..128 {
            let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let temp_path = parent.join(format!(
                "{TEMP_FILE_PREFIX}{label}-{}-{unique}.tmp",
                std::process::id()
            ));

            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&temp_path)
            {
                Ok(file) => {
                    return Ok(Self {
                        target_path,
                        temp_path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to reserve a unique neo-rimage temporary output",
        ))
    }

    pub fn writer(&mut self) -> io::Result<&mut File> {
        self.file.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "output transaction writer is already closed",
            )
        })
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    pub fn sync(&mut self) -> io::Result<u64> {
        let file = self.writer()?;
        file.flush()?;
        file.sync_all()?;
        let length = file.metadata()?.len();
        if length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encoder produced an empty output",
            ));
        }
        Ok(length)
    }

    pub fn commit(mut self, policy: CommitPolicy) -> io::Result<CommitOutcome> {
        self.close_writer();

        let outcome = match policy {
            CommitPolicy::FailIfExists => self.commit_new_target(),
            CommitPolicy::Replace { backup_path } => self.commit_replace(backup_path),
        }?;

        self.committed = true;
        Ok(outcome)
    }

    fn close_writer(&mut self) {
        drop(self.file.take());
    }

    fn commit_new_target(&mut self) -> io::Result<CommitOutcome> {
        // Creating a hard link is an atomic "create if absent" operation. A plain
        // exists check followed by rename has a TOCTOU window and can overwrite a
        // file another process created between those calls.
        fs::hard_link(&self.temp_path, &self.target_path)?;

        let mut outcome = CommitOutcome::default();
        if let Err(error) = fs::remove_file(&self.temp_path) {
            outcome.cleanup_warnings.push(format!(
                "committed output but could not remove temporary link {}: {error}",
                self.temp_path.display()
            ));
        }
        Ok(outcome)
    }

    fn commit_replace(&mut self, backup_path: Option<PathBuf>) -> io::Result<CommitOutcome> {
        if !self.target_path.exists() {
            match self.commit_new_target() {
                Ok(outcome) => return Ok(outcome),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    // Another writer won the race after the existence check. Fall
                    // through to the replace/backup path using the now-existing file.
                }
                Err(error) => return Err(error),
            }
        }

        let rollback_path = match backup_path.as_ref() {
            Some(path) => {
                if path.exists() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("backup target already exists: {}", path.display()),
                    ));
                }
                if parent_or_current(path) != parent_or_current(&self.target_path) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "backup must be on the same filesystem as the output target",
                    ));
                }
                path.clone()
            }
            None => reserve_rollback_path(&self.target_path)?,
        };

        // Preserve the previous target before opening the replacement window.
        // A hard link is atomic and cannot clobber an independently-created
        // backup path. It also lets us restore the previous inode if commit fails.
        fs::hard_link(&self.target_path, &rollback_path)?;
        if let Err(error) = fs::remove_file(&self.target_path) {
            let _ = fs::remove_file(&rollback_path);
            return Err(error);
        }

        let committed = match self.commit_new_target() {
            Ok(outcome) => outcome,
            Err(commit_error) => {
                // `hard_link` restores only when the target is still absent. If an
                // external writer created it during the replacement window, never
                // overwrite that file; preserve the previous output at rollback_path.
                let rollback_result = fs::hard_link(&rollback_path, &self.target_path);
                return match rollback_result {
                    Ok(()) => {
                        let _ = fs::remove_file(&rollback_path);
                        Err(commit_error)
                    }
                    Err(rollback_error) => Err(io::Error::new(
                        commit_error.kind(),
                        format!(
                            "failed to commit replacement ({commit_error}); rollback also failed ({rollback_error}); previous output remains at {}",
                            rollback_path.display()
                        ),
                    )),
                };
            }
        };

        let mut outcome = CommitOutcome {
            backup_path: backup_path.clone(),
            cleanup_warnings: committed.cleanup_warnings,
        };

        if backup_path.is_none() {
            if let Err(error) = fs::remove_file(&rollback_path) {
                outcome.cleanup_warnings.push(format!(
                    "replacement committed but previous output cleanup failed at {}: {error}",
                    rollback_path.display()
                ));
            }
        }

        Ok(outcome)
    }
}

impl Drop for OutputTransaction {
    fn drop(&mut self) {
        self.close_writer();
        if !self.committed {
            let _ = fs::remove_file(&self.temp_path);
        }
    }
}

fn reserve_rollback_path(target: &Path) -> io::Result<PathBuf> {
    let parent = parent_or_current(target);
    let label = target
        .file_name()
        .and_then(|name| name.to_str())
        .map(safe_file_label)
        .unwrap_or_else(|| "output".to_owned());

    for _ in 0..128 {
        let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "{TEMP_FILE_PREFIX}rollback-{label}-{}-{unique}.tmp",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to reserve a rollback path",
    ))
}

fn parent_or_current(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn safe_file_label(value: &str) -> String {
    let mut result = value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                Some(character)
            } else {
                None
            }
        })
        .take(48)
        .collect::<String>();

    if result.is_empty() {
        result.push_str("item");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "neo-rimage-engine-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn target_is_invisible_until_commit() {
        let directory = TestDirectory::new("atomic");
        let target = directory.join("result.jpg");
        let mut transaction =
            OutputTransaction::begin(&target, "item/unsafe").expect("begin output transaction");

        transaction
            .writer()
            .expect("temporary writer")
            .write_all(b"encoded")
            .expect("write output");
        transaction.sync().expect("sync output");

        assert!(!target.exists());
        assert_eq!(transaction.temp_path().parent(), target.parent());

        transaction
            .commit(CommitPolicy::FailIfExists)
            .expect("commit output");
        assert_eq!(fs::read(target).expect("read committed output"), b"encoded");
    }

    #[test]
    fn dropping_transaction_removes_temporary_output() {
        let directory = TestDirectory::new("drop");
        let target = directory.join("result.jpg");
        let temp_path = {
            let mut transaction =
                OutputTransaction::begin(&target, "item").expect("begin output transaction");
            transaction
                .writer()
                .expect("temporary writer")
                .write_all(b"partial")
                .expect("write output");
            transaction.temp_path().to_path_buf()
        };

        assert!(!temp_path.exists());
        assert!(!target.exists());
    }

    #[test]
    fn fail_policy_preserves_existing_target() {
        let directory = TestDirectory::new("collision");
        let target = directory.join("result.jpg");
        fs::write(&target, b"existing").expect("seed existing output");

        let mut transaction =
            OutputTransaction::begin(&target, "item").expect("begin output transaction");
        transaction
            .writer()
            .expect("temporary writer")
            .write_all(b"new")
            .expect("write output");
        transaction.sync().expect("sync output");

        let error = transaction
            .commit(CommitPolicy::FailIfExists)
            .expect_err("collision should fail");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(target).expect("read existing output"), b"existing");
    }

    #[test]
    fn replace_policy_can_keep_an_explicit_backup() {
        let directory = TestDirectory::new("backup");
        let target = directory.join("result.jpg");
        let backup = directory.join("result.previous.jpg");
        fs::write(&target, b"existing").expect("seed existing output");

        let mut transaction =
            OutputTransaction::begin(&target, "item").expect("begin output transaction");
        transaction
            .writer()
            .expect("temporary writer")
            .write_all(b"new")
            .expect("write output");
        transaction.sync().expect("sync output");

        let outcome = transaction
            .commit(CommitPolicy::Replace {
                backup_path: Some(backup.clone()),
            })
            .expect("replace output");

        assert_eq!(outcome.backup_path, Some(backup.clone()));
        assert_eq!(fs::read(target).expect("read replacement"), b"new");
        assert_eq!(fs::read(backup).expect("read backup"), b"existing");
    }

    #[test]
    fn sync_rejects_empty_encoder_output() {
        let directory = TestDirectory::new("empty");
        let target = directory.join("result.jpg");
        let mut transaction =
            OutputTransaction::begin(&target, "item").expect("begin output transaction");

        let error = transaction.sync().expect_err("empty output should fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
