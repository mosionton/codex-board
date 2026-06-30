# Session Tree Glyphs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing Sessions tree view visually obvious by showing tree glyphs in the `source` column.

**Architecture:** `SessionsState` remains responsible for filtered visible rows and tree ordering, and will additionally compute a per-row tree prefix string. `src/ui/tables.rs` will consume the prefix in Tree mode and keep Flat mode free of tree glyphs.

**Tech Stack:** Rust, ratatui, cargo test.

---

## File Structure

- Modify `src/app/sessions_state.rs`: store and expose visible row tree prefixes.
- Modify `src/ui/tables.rs`: combine tree prefix and source metadata in Tree view; keep Flat view plain.
- Modify `src/ui/mod.rs`: add rendering helper tests for Tree and Flat source labels.
- Modify `README.md`: mention tree glyphs in source column.

## Task 1: State Tree Prefixes

**Files:**
- Modify: `src/app/sessions_state.rs`

- [ ] **Step 1: Write failing state tests**

Add tests in `src/app/sessions_state.rs`:

```rust
#[test]
fn tree_view_builds_visible_tree_prefixes() {
    let current_dir = PathBuf::from("/repo/current");
    let mut parent = test_session("parent", current_dir.clone(), "alpha", "parent");
    parent.timestamp = "2026-06-24T10:00:00Z".into();
    let mut first_child = test_session("first-child", current_dir.clone(), "alpha", "first");
    first_child.timestamp = "2026-06-24T09:00:00Z".into();
    first_child.thread_source = "subagent".into();
    first_child.parent_thread_id = Some("parent".into());
    let mut grandchild = test_session("grandchild", current_dir.clone(), "alpha", "grandchild");
    grandchild.timestamp = "2026-06-24T08:00:00Z".into();
    grandchild.thread_source = "subagent".into();
    grandchild.parent_thread_id = Some("first-child".into());
    let mut last_child = test_session("last-child", current_dir.clone(), "alpha", "last");
    last_child.timestamp = "2026-06-24T07:00:00Z".into();
    last_child.thread_source = "subagent".into();
    last_child.parent_thread_id = Some("parent".into());

    let mut state = SessionsState::new(
        vec![parent, first_child, grandchild, last_child],
        current_dir,
        PathBuf::from("sessions"),
    );
    state.refresh_visible();

    assert_eq!(state.visible_session(0).unwrap().id, "parent");
    assert_eq!(state.visible_tree_prefix(0), "● ");
    assert_eq!(state.visible_session(1).unwrap().id, "first-child");
    assert_eq!(state.visible_tree_prefix(1), "├─ ");
    assert_eq!(state.visible_session(2).unwrap().id, "grandchild");
    assert_eq!(state.visible_tree_prefix(2), "│  └─ ");
    assert_eq!(state.visible_session(3).unwrap().id, "last-child");
    assert_eq!(state.visible_tree_prefix(3), "└─ ");
}

#[test]
fn flat_view_has_empty_tree_prefixes() {
    let current_dir = PathBuf::from("/repo/current");
    let mut parent = test_session("parent", current_dir.clone(), "alpha", "parent");
    parent.timestamp = "2026-06-24T10:00:00Z".into();
    let mut child = test_session("child", current_dir.clone(), "alpha", "child");
    child.timestamp = "2026-06-24T09:00:00Z".into();
    child.thread_source = "subagent".into();
    child.parent_thread_id = Some("parent".into());

    let mut state = SessionsState::new(vec![parent, child], current_dir, PathBuf::from("sessions"));
    state.toggle_view_mode();
    state.refresh_visible();

    assert_eq!(state.view_mode(), SessionViewMode::Flat);
    assert_eq!(state.visible_tree_prefix(0), "");
    assert_eq!(state.visible_tree_prefix(1), "");
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test app::sessions_state::tests::tree_view_builds_visible_tree_prefixes --all-features --locked
cargo test app::sessions_state::tests::flat_view_has_empty_tree_prefixes --all-features --locked
```

Expected: FAIL because `visible_tree_prefix` does not exist.

- [ ] **Step 3: Implement prefix storage and traversal**

Add field:

```rust
pub(super) visible_tree_prefixes: Vec<String>,
```

Initialize it in `SessionsState::new`.

Add accessor:

```rust
pub(crate) fn visible_tree_prefix(&self, visible_index: usize) -> &str {
    self.visible_tree_prefixes
        .get(visible_index)
        .map_or("", String::as_str)
}
```

Update `rebuild_flat_visible`:

```rust
self.visible_tree_prefixes = vec![String::new(); candidates.len()];
```

Replace `append_tree_row` with a version that accepts `prefix: String` and `is_last: bool`, pushes `● ` for roots, and for children builds `├─ `, `└─ `, and ancestor continuation strings (`│  ` or three spaces).

- [ ] **Step 4: Run state tests**

Run:

```bash
cargo test app::sessions_state::tests --all-features --locked
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/sessions_state.rs
git commit -m "feat: 生成会话树符号前缀"
```

## Task 2: Render Tree Glyphs In Source Column

**Files:**
- Modify: `src/ui/tables.rs`
- Modify: `src/ui/mod.rs`
- Modify: `README.md`

- [ ] **Step 1: Write failing rendering tests**

In `src/ui/tables.rs`, make `session_source_label` visible to UI tests:

```rust
pub(super) fn session_source_label(
    session: &Session,
    view_mode: SessionViewMode,
    tree_prefix: &str,
) -> String
```

Add tests in `src/ui/mod.rs`:

```rust
#[test]
fn session_source_label_shows_tree_glyphs_only_in_tree_view() {
    let cwd = PathBuf::from("/repo/current");
    let mut child = test_session("child", cwd, "switcher", "summary");
    child.thread_source = "subagent".to_string();
    child.parent_thread_id = Some("parent".to_string());
    child.agent_nickname = Some("Boole".to_string());
    child.agent_role = Some("worker".to_string());

    assert_eq!(
        tables::session_source_label(&child, SessionViewMode::Tree, "├─ "),
        "├─ sub Boole/worker"
    );
    assert_eq!(
        tables::session_source_label(&child, SessionViewMode::Flat, "├─ "),
        "sub Boole/worker"
    );
}

#[test]
fn orphan_source_label_keeps_parent_id_without_tree_depth() {
    let cwd = PathBuf::from("/repo/current");
    let mut child = test_session("child", cwd, "switcher", "summary");
    child.thread_source = "subagent".to_string();
    child.parent_thread_id = Some("019f1067-10b5-7d02-8176-093dbc9170fa".to_string());
    child.agent_nickname = Some("Boole".to_string());

    assert_eq!(
        tables::session_source_label(&child, SessionViewMode::Tree, "● "),
        "● sub Boole <- 019f1067"
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test ui::tests::session_source_label_shows_tree_glyphs_only_in_tree_view --all-features --locked
cargo test ui::tests::orphan_source_label_keeps_parent_id_without_tree_depth --all-features --locked
```

Expected: FAIL because `session_source_label` has the old signature and does not use view mode.

- [ ] **Step 3: Implement table rendering**

Update imports in `src/ui/tables.rs`:

```rust
use crate::{
    app::{App, SessionViewMode},
    session_store::{Session, truncate_chars},
};
```

In `draw_sessions`, call:

```rust
let source = session_source_label(
    session,
    app.session_state.view_mode(),
    app.session_state.visible_tree_prefix(index),
);
Cell::from(truncate_chars(&source, 24))
```

Implement source label as:

```rust
pub(super) fn session_source_label(
    session: &Session,
    view_mode: SessionViewMode,
    tree_prefix: &str,
) -> String {
    let is_subagent = session.thread_source == "subagent" || session.parent_thread_id.is_some();
    let mut label = if is_subagent {
        subagent_source_label(session)
    } else {
        session.thread_source.clone()
    };
    if is_orphan_root(session, view_mode, tree_prefix)
        && let Some(parent) = session.parent_thread_id.as_deref()
    {
        label.push_str(" <- ");
        label.push_str(&short_session_id(parent));
    }
    match view_mode {
        SessionViewMode::Tree => format!("{tree_prefix}{label}"),
        SessionViewMode::Flat => label,
    }
}
```

Keep helper functions small: `subagent_source_label`, `is_orphan_root`, `short_session_id`.

- [ ] **Step 4: Update README**

Change the Sessions feature text to say subagent sessions are shown with tree glyphs in the `source` column.

- [ ] **Step 5: Run UI tests**

Run:

```bash
cargo test ui:: --all-features --locked
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/ui/tables.rs src/ui/mod.rs README.md
git commit -m "feat: 显示明显会话树符号"
```

## Task 3: Full Verification

**Files:**
- No source edits expected.

- [ ] **Step 1: Format check**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS. If it fails only for formatting, run `cargo fmt --all`, then re-run the check.

- [ ] **Step 2: Full test suite**

Run:

```bash
cargo test --all-features --locked
```

Expected: PASS.

- [ ] **Step 3: Diff hygiene**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 4: Commit formatting-only changes if needed**

If `cargo fmt --all` changed files:

```bash
git add <changed-files>
git commit -m "chore: 整理会话树符号格式"
```

If no files changed, skip this commit.

## Self-Review Notes

- Spec coverage: visible tree glyphs are covered by Task 1 and Task 2.
- Flat view no-tree behavior is tested at both state and rendering levels.
- No expand/collapse or extra tree column is introduced.
