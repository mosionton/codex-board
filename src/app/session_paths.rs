use std::path::Path;

pub fn session_matches_current_dir(session_cwd: &Path, current_dir: &Path) -> bool {
    if session_cwd == current_dir {
        return true;
    }

    let Ok(session_cwd) = std::fs::canonicalize(session_cwd) else {
        return false;
    };
    let Ok(current_dir) = std::fs::canonicalize(current_dir) else {
        return false;
    };

    session_cwd == current_dir
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::app::session_matches_current_dir;

    #[test]
    fn equal_missing_paths_match_without_canonicalizing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing");

        assert!(session_matches_current_dir(&path, &path));
    }

    #[test]
    fn canonical_equivalent_existing_paths_match() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir(&project).unwrap();
        let dotted_project = project.join(".");

        assert!(session_matches_current_dir(&project, &dotted_project));
    }

    #[test]
    fn distinct_missing_paths_do_not_match() {
        let dir = tempdir().unwrap();

        assert!(!session_matches_current_dir(
            dir.path().join("missing-session").as_path(),
            dir.path().join("missing-current").as_path(),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_current_dir_matches_real_session_cwd() {
        let dir = tempdir().unwrap();
        let real_project = dir.path().join("real-project");
        let linked_project = dir.path().join("linked-project");
        fs::create_dir(&real_project).unwrap();
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();

        assert!(session_matches_current_dir(&real_project, &linked_project));
    }
}
