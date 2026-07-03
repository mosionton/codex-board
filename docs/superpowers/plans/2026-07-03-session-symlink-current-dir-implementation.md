# Session Symlink Current Dir Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Sessions current-directory scope treat symlink-equivalent paths as the same workspace without changing displayed or resumed paths.

**Architecture:** Add one shared path matcher in the `app` layer, then replace each direct `session.cwd == current_dir` current-scope check with that matcher. Keep `Session` unchanged so UI display and `codex resume` keep using the original session `cwd`.

**Tech Stack:** Rust 2024, `std::fs::canonicalize`, Ratatui app state modules, `tempfile` for tests.

---

## File Structure

- Create `src/app/session_paths.rs`: owns current-directory path equivalence logic and its focused unit tests.
- Modify `src/app/mod.rs`: registers and re-exports the matcher for app and UI modules.
- Modify `src/app/sessions_state.rs`: uses the matcher for Sessions visible row filtering.
- Modify `src/app/provider_tabs.rs`: uses the matcher when building provider tabs for current-directory scope.
- Modify `src/ui/search_dialogs.rs`: uses the matcher for session search match counts.
- Modify `README.md`: documents that current-directory Sessions scope recognizes symlink-equivalent paths.

## Task 1: Add Shared Session Path Matcher

**Files:**
- Create: `src/app/session_paths.rs`
- Modify: `src/app/mod.rs`

- [ ] **Step 1: Write the failing tests and module registration**

Create `src/app/session_paths.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::session_matches_current_dir;

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
```

In `src/app/mod.rs`, add the module declaration near the other `mod` lines:

```rust
mod session_paths;
```

Do not add the function implementation yet.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test app::session_paths::tests --all-features --locked
```

Expected: FAIL to compile because `session_matches_current_dir` is not defined.

- [ ] **Step 3: Implement the matcher**

Replace `src/app/session_paths.rs` with:

```rust
use std::path::Path;

pub(crate) fn session_matches_current_dir(session_cwd: &Path, current_dir: &Path) -> bool {
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
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::session_matches_current_dir;

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
```

In `src/app/mod.rs`, re-export the matcher near the other `pub(crate) use` lines:

```rust
pub(crate) use session_paths::session_matches_current_dir;
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
cargo test app::session_paths::tests --all-features --locked
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add src/app/mod.rs src/app/session_paths.rs
git commit -m "feat: 增加会话目录等价匹配" -m "新增共享路径匹配逻辑，用于识别软连接等价的当前目录。" -m "- 优先保留原有字面路径匹配" -m "- 路径存在时使用 canonicalize 比较真实路径" -m "- 路径不存在时保持严格不匹配行为"
```

## Task 2: Use Matcher in Sessions Visible Filtering

**Files:**
- Modify: `src/app/sessions_state.rs`

- [ ] **Step 1: Write the failing test**

Inside `#[cfg(test)] mod tests` in `src/app/sessions_state.rs`, add the `tempfile` import:

```rust
use tempfile::tempdir;
```

Add this test:

```rust
#[test]
fn refresh_visible_matches_canonical_equivalent_current_dir() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let current_dir = project.join(".");
    let mut state = SessionsState::new(
        vec![test_session("1", project, "alpha", "first request")],
        current_dir,
        PathBuf::from("sessions"),
    );

    state.refresh_visible();

    assert_eq!(state.visible_len(), 1);
    assert_eq!(state.selected_session().unwrap().id, "1");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```sh
cargo test app::sessions_state::tests::refresh_visible_matches_canonical_equivalent_current_dir --all-features --locked
```

Expected: FAIL because direct path equality filters the session out.

- [ ] **Step 3: Use the matcher in `refresh_visible`**

Update the import near the top of `src/app/sessions_state.rs`:

```rust
use super::{ProviderTabs, Scope, SearchState, TableSelection, session_matches_current_dir};
```

Replace the current-directory filter in `refresh_visible` with:

```rust
.filter(|(_, session)| match self.scope {
    Scope::CurrentDir => session_matches_current_dir(&session.cwd, &self.current_dir),
    Scope::All => true,
})
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
cargo test app::sessions_state::tests::refresh_visible_matches_canonical_equivalent_current_dir --all-features --locked
cargo test app::sessions_state::tests --all-features --locked
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add src/app/sessions_state.rs
git commit -m "fix: 支持软连接目录过滤会话" -m "让 Sessions 当前目录范围使用共享目录等价判断。" -m "- 当前目录路径等价时显示对应 session" -m "- 全部会话范围和树形排序保持不变"
```

## Task 3: Use Matcher in Provider Tabs

**Files:**
- Modify: `src/app/provider_tabs.rs`

- [ ] **Step 1: Write the failing test**

At the end of `src/app/provider_tabs.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::*;

    fn test_session(id: &str, cwd: PathBuf, provider: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: "summary".to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

    #[test]
    fn current_dir_tabs_include_canonical_equivalent_session_provider() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir(&project).unwrap();
        let current_dir = project.join(".");
        let sessions = vec![
            test_session("1", project, "alpha"),
            test_session("2", dir.path().join("other"), "beta"),
        ];

        assert_eq!(
            ProviderTabs::new(&sessions, Scope::CurrentDir, &current_dir).labels(),
            vec!["All".to_string(), "alpha".to_string()]
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```sh
cargo test app::provider_tabs::tests::current_dir_tabs_include_canonical_equivalent_session_provider --all-features --locked
```

Expected: FAIL because `build_labels` still uses direct path equality.

- [ ] **Step 3: Use the matcher in `build_labels`**

Update the import in `src/app/provider_tabs.rs`:

```rust
use super::{Scope, cycle_index, session_matches_current_dir};
```

Replace the current-directory check in `build_labels` with:

```rust
if scope == Scope::CurrentDir && !session_matches_current_dir(&session.cwd, current_dir) {
    continue;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
cargo test app::provider_tabs::tests::current_dir_tabs_include_canonical_equivalent_session_provider --all-features --locked
cargo test app::tests::filters_provider_tabs_by_scope --all-features --locked
cargo test app::provider_tabs::tests --all-features --locked
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add src/app/provider_tabs.rs
git commit -m "fix: 支持软连接目录 provider 过滤" -m "让 provider tabs 在当前目录范围下复用会话目录等价判断。" -m "- 软连接等价路径会纳入当前目录 provider 标签" -m "- 全部会话范围仍展示所有 provider"
```

## Task 4: Use Matcher in Session Search Count

**Files:**
- Modify: `src/ui/search_dialogs.rs`

- [ ] **Step 1: Write the failing test**

Inside `#[cfg(test)] mod tests` in `src/ui/search_dialogs.rs`, add:

```rust
use tempfile::tempdir;
```

Add this test:

```rust
#[test]
fn session_search_match_count_matches_canonical_equivalent_current_dir() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let current_dir = project.join(".");
    let sessions = vec![test_session("1", project, "alpha", "first request")];
    let app = app_with_sessions(sessions, current_dir);

    assert_eq!(session_search_match_count(&app, "request"), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```sh
cargo test ui::search_dialogs::tests::session_search_match_count_matches_canonical_equivalent_current_dir --all-features --locked
```

Expected: FAIL because `session_search_match_count` still uses direct path equality.

- [ ] **Step 3: Use the matcher in `session_search_match_count`**

Update the app import in `src/ui/search_dialogs.rs`:

```rust
app::{App, Scope, session_matches_current_dir},
```

Replace the current-directory filter with:

```rust
.filter(|session| match app.session_state.scope() {
    Scope::CurrentDir => {
        session_matches_current_dir(&session.cwd, app.session_state.current_dir())
    }
    Scope::All => true,
})
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
cargo test ui::search_dialogs::tests::session_search_match_count_matches_canonical_equivalent_current_dir --all-features --locked
cargo test ui::search_dialogs::tests --all-features --locked
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add src/ui/search_dialogs.rs
git commit -m "fix: 支持软连接目录搜索计数" -m "让 Sessions 搜索弹窗计数复用当前目录等价判断。" -m "- 搜索草稿匹配数与列表过滤保持一致" -m "- 搜索字段和 provider 过滤语义保持不变"
```

## Task 5: Document and Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

In `README.md`, under the Sessions feature list, replace:

```markdown
- 当前目录和全部会话范围切换。
```

with:

```markdown
- 当前目录和全部会话范围切换，当前目录范围会识别软连接等价路径。
```

- [ ] **Step 2: Run full verification**

Run:

```sh
cargo test --all-targets --all-features --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Expected: all commands PASS with no warnings.

- [ ] **Step 3: Commit**

```sh
git add README.md
git commit -m "chore: 说明软连接目录会话匹配" -m "补充 Sessions 当前目录范围的软连接匹配说明。" -m "- README 描述当前目录范围识别软连接等价路径" -m "- 完成测试、格式和 clippy 验证"
```

## Final Checks

- [ ] Run `git status --short` and confirm only unrelated pre-existing files remain, such as `.DS_Store`.
- [ ] Confirm `git log --oneline -5` shows the planned commits in order.
- [ ] Summarize changed behavior: current-directory matching recognizes canonical-equivalent paths, while displayed `cwd` and resume directory remain original.
