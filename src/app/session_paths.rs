use std::path::{Path, PathBuf};

pub struct CurrentDirMatcher<'a> {
    current_dir: &'a Path,
    canonical_current_dir: Option<PathBuf>,
}

impl<'a> CurrentDirMatcher<'a> {
    #[must_use]
    pub fn new(current_dir: &'a Path) -> Self {
        Self {
            current_dir,
            canonical_current_dir: std::fs::canonicalize(current_dir).ok(),
        }
    }

    #[must_use]
    pub fn matches(&self, session_cwd: &Path) -> bool {
        if session_cwd.as_os_str() == self.current_dir.as_os_str() {
            return true;
        }

        let Some(current_dir) = self.canonical_current_dir.as_deref() else {
            return false;
        };
        let Ok(session_cwd) = std::fs::canonicalize(session_cwd) else {
            return false;
        };

        session_cwd == current_dir
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::app::CurrentDirMatcher;

    #[test]
    fn equal_missing_paths_match_without_canonicalizing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing");
        let matcher = CurrentDirMatcher::new(&path);

        assert!(matcher.matches(&path));
    }

    #[test]
    fn canonical_equivalent_existing_paths_match() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir(&project).unwrap();
        let dotted_project = project.join(".");
        let matcher = CurrentDirMatcher::new(&dotted_project);

        assert!(matcher.matches(&project));
    }

    #[test]
    fn distinct_missing_paths_do_not_match() {
        let dir = tempdir().unwrap();
        let session_cwd = dir.path().join("missing-session");
        let current_dir = dir.path().join("missing-current");
        let matcher = CurrentDirMatcher::new(&current_dir);

        assert!(!matcher.matches(&session_cwd));
    }

    #[test]
    fn different_missing_paths_with_dot_do_not_match() {
        let dir = tempdir().unwrap();
        let current_dir = dir.path().join("missing");
        let session_cwd = current_dir.join(".");
        let matcher = CurrentDirMatcher::new(&current_dir);

        assert!(!matcher.matches(&session_cwd));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_current_dir_matches_real_session_cwd() {
        let dir = tempdir().unwrap();
        let real_project = dir.path().join("real-project");
        let linked_project = dir.path().join("linked-project");
        fs::create_dir(&real_project).unwrap();
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();
        let matcher = CurrentDirMatcher::new(&linked_project);

        assert!(matcher.matches(&real_project));
    }
}
