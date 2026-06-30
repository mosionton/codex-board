# Session Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Sessions default to a parent/child tree, expose subagent metadata, and allow `v` to toggle Tree/Flat views.

**Architecture:** Extend `Session` with structured relationship metadata parsed from `session_meta`. Keep filtering behavior in `SessionsState`, then generate flat or tree-ordered visible rows plus per-row depth/source display data. Update TUI rendering and input as thin consumers of that state.

**Tech Stack:** Rust, serde_json, ratatui, crossterm, cargo test.

---

## File Structure

- Modify `src/session_store.rs`: parse relation metadata and include it in search text.
- Modify `src/app/sessions_state.rs`: add `SessionViewMode`, visible row depth, tree ordering, and view toggling.
- Modify `src/app/sessions.rs`: expose toggle behavior through `App`.
- Modify `src/app/mod.rs`: re-export the view mode for UI/tests.
- Modify `src/ui/page_input.rs`: route `v` on Sessions to the view toggle.
- Modify `src/ui/tables.rs`: add `source` column and indentation.
- Modify `src/ui/details.rs`: add relation fields to session details.
- Modify `src/ui/chrome.rs`: show current view mode and footer shortcut.
- Modify `src/ui/search_dialogs.rs`: keep match counting aligned with expanded search text.
- Modify `src/ui/input.rs`: add shortcut regression tests.
- Modify `README.md`: document tree default and `v` shortcut.

## Task 1: Parse Session Relationship Metadata

**Files:**
- Modify: `src/session_store.rs`

- [ ] **Step 1: Write failing parser/search tests**

Add tests in `src/session_store.rs`:

```rust
#[test]
fn parses_subagent_metadata_from_session_meta() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    fs::write(
        &path,
        r#"{"timestamp":"2026-06-29T11:34:49Z","type":"session_meta","payload":{"id":"child","session_id":"parent","parent_thread_id":"parent","timestamp":"2026-06-29T11:34:49Z","cwd":"/tmp/project","model_provider":"switcher","thread_source":"subagent","agent_nickname":"Boole","agent_role":"worker","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent","depth":1,"agent_nickname":"Boole","agent_role":"worker"}}}}}"#
            .to_string()
            + "\n",
    )
    .unwrap();

    let session = parse_session_file(&path).unwrap().unwrap();

    assert_eq!(session.id, "child");
    assert_eq!(session.thread_source, "subagent");
    assert_eq!(session.parent_thread_id.as_deref(), Some("parent"));
    assert_eq!(session.agent_nickname.as_deref(), Some("Boole"));
    assert_eq!(session.agent_role.as_deref(), Some("worker"));
    assert_eq!(session.agent_depth, Some(1));
}

#[test]
fn search_matches_session_relationship_metadata() {
    let mut session = test_session_for_search();
    session.thread_source = "subagent".into();
    session.parent_thread_id = Some("parent-123".into());
    session.agent_nickname = Some("Boole".into());
    session.agent_role = Some("worker".into());

    assert!(matches_search(&session, &search_terms("parent-123 boole")));
    assert!(matches_search(&session, &search_terms("subagent worker")));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test session_store::tests::parses_subagent_metadata_from_session_meta session_store::tests::search_matches_session_relationship_metadata --all-features --locked
```

Expected: FAIL because `Session` has no relationship fields.

- [ ] **Step 3: Implement metadata fields and extraction**

Add fields to `Session`:

```rust
pub(super) thread_source: String,
pub(super) parent_thread_id: Option<String>,
pub(super) agent_nickname: Option<String>,
pub(super) agent_role: Option<String>,
pub(super) agent_depth: Option<u32>,
```

In `parse_session_file`, read these from `session_meta.payload`, with fallback to `payload.source.subagent.thread_spawn`. Add helper functions:

```rust
fn subagent_spawn(payload: &JsonValue) -> Option<&JsonValue> {
    payload.get("source")?.get("subagent")?.get("thread_spawn")
}

fn optional_string_from(primary: Option<&JsonValue>, fallback: Option<&JsonValue>) -> Option<String> {
    primary
        .and_then(JsonValue::as_str)
        .or_else(|| fallback.and_then(JsonValue::as_str))
        .map(str::to_string)
}
```

Include the new fields in `session_search_text`.

- [ ] **Step 4: Update test fixtures**

Every test-created `Session` must include:

```rust
thread_source: "user".into(),
parent_thread_id: None,
agent_nickname: None,
agent_role: None,
agent_depth: None,
```

- [ ] **Step 5: Run parser tests and commit**

Run:

```bash
cargo test session_store::tests --all-features --locked
```

Expected: PASS.

Commit:

```bash
git add src/session_store.rs
git commit -m "feat: 解析会话派发关系"
```

## Task 2: Build Tree/Flat Session State

**Files:**
- Modify: `src/app/sessions_state.rs`
- Modify: `src/app/sessions.rs`
- Modify: `src/app/mod.rs`

- [ ] **Step 1: Write failing state tests**

Add tests in `src/app/sessions_state.rs`:

```rust
#[test]
fn sessions_default_to_tree_view_and_order_children_after_parent() {
    let current_dir = PathBuf::from("/repo/current");
    let mut parent = test_session("parent", current_dir.clone(), "alpha", "parent");
    parent.timestamp = "2026-06-24T10:00:00Z".into();
    let mut child = test_session("child", current_dir.clone(), "alpha", "child");
    child.timestamp = "2026-06-24T11:00:00Z".into();
    child.thread_source = "subagent".into();
    child.parent_thread_id = Some("parent".into());
    child.agent_nickname = Some("Boole".into());

    let mut state = SessionsState::new(vec![child, parent], current_dir, PathBuf::from("sessions"));
    state.refresh_visible();

    assert_eq!(state.view_mode(), SessionViewMode::Tree);
    assert_eq!(state.visible_session(0).unwrap().id, "parent");
    assert_eq!(state.visible_depth(0), 0);
    assert_eq!(state.visible_session(1).unwrap().id, "child");
    assert_eq!(state.visible_depth(1), 1);
}

#[test]
fn flat_view_keeps_timestamp_order() {
    let current_dir = PathBuf::from("/repo/current");
    let mut parent = test_session("parent", current_dir.clone(), "alpha", "parent");
    parent.timestamp = "2026-06-24T10:00:00Z".into();
    let mut child = test_session("child", current_dir.clone(), "alpha", "child");
    child.timestamp = "2026-06-24T11:00:00Z".into();
    child.thread_source = "subagent".into();
    child.parent_thread_id = Some("parent".into());

    let mut state = SessionsState::new(vec![child, parent], current_dir, PathBuf::from("sessions"));
    state.toggle_view_mode();
    state.refresh_visible();

    assert_eq!(state.view_mode(), SessionViewMode::Flat);
    assert_eq!(state.visible_session(0).unwrap().id, "child");
    assert_eq!(state.visible_depth(0), 0);
    assert_eq!(state.visible_session(1).unwrap().id, "parent");
}

#[test]
fn tree_view_keeps_orphan_child_visible_at_root() {
    let current_dir = PathBuf::from("/repo/current");
    let mut child = test_session("child", current_dir.clone(), "alpha", "child");
    child.thread_source = "subagent".into();
    child.parent_thread_id = Some("missing-parent".into());

    let mut state = SessionsState::new(vec![child], current_dir, PathBuf::from("sessions"));
    state.refresh_visible();

    assert_eq!(state.visible_session(0).unwrap().id, "child");
    assert_eq!(state.visible_depth(0), 0);
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test app::sessions_state::tests::sessions_default_to_tree_view_and_order_children_after_parent app::sessions_state::tests::flat_view_keeps_timestamp_order app::sessions_state::tests::tree_view_keeps_orphan_child_visible_at_root --all-features --locked
```

Expected: FAIL because `SessionViewMode`, `visible_depth`, and tree ordering do not exist.

- [ ] **Step 3: Implement state model**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionViewMode {
    Tree,
    Flat,
}
```

Add fields:

```rust
pub(super) visible_depths: Vec<usize>,
pub(super) view_mode: SessionViewMode,
```

Add methods:

```rust
pub(crate) const fn view_mode(&self) -> SessionViewMode { self.view_mode }
pub(crate) fn toggle_view_mode(&mut self) { ... }
pub(crate) fn visible_depth(&self, visible_index: usize) -> usize { ... }
```

Refactor `refresh_visible` to build candidate indices first, then call `build_flat_visible` or `build_tree_visible`. Tree ordering must output roots first, then depth-first children, use timestamp-desc sibling order, keep orphans at root, and track visited indices to avoid loops.

- [ ] **Step 4: Add App toggle wrapper**

In `src/app/sessions.rs`:

```rust
pub(crate) fn toggle_session_view_mode(&mut self) {
    self.session_state.toggle_view_mode();
    self.session_state.reset_selection();
    self.refresh_visible();
}
```

Re-export `SessionViewMode` from `src/app/mod.rs`.

- [ ] **Step 5: Run state tests and commit**

Run:

```bash
cargo test app::sessions_state::tests --all-features --locked
```

Expected: PASS.

Commit:

```bash
git add src/app/sessions_state.rs src/app/sessions.rs src/app/mod.rs
git commit -m "feat: 构建会话树视图状态"
```

## Task 3: Wire Input and TUI Rendering

**Files:**
- Modify: `src/ui/page_input.rs`
- Modify: `src/ui/tables.rs`
- Modify: `src/ui/details.rs`
- Modify: `src/ui/chrome.rs`
- Modify: `src/ui/input.rs`

- [ ] **Step 1: Write failing UI tests**

Add tests:

```rust
#[test]
fn v_key_toggles_session_view_mode() {
    let mut app = app_with_registry(ProviderRegistry::default());
    assert_eq!(app.session_state.view_mode(), SessionViewMode::Tree);

    let action = handle_sessions_page_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
    );

    assert!(action.is_none());
    assert_eq!(app.session_state.view_mode(), SessionViewMode::Flat);
}
```

In `src/ui/details.rs` tests, add a selected session with relationship metadata and assert `selected_session_details` includes `source`, `parent`, `agent`, `role`, and `depth`.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test ui::input::tests::v_key_toggles_session_view_mode ui::tests::selected_details_render_fallbacks_and_provider_values --all-features --locked
```

Expected: FAIL because `v` is not handled and details do not include relation fields.

- [ ] **Step 3: Implement input and rendering**

In `src/ui/page_input.rs`, add:

```rust
(KeyCode::Char('v'), KeyModifiers::NONE) => {
    app.toggle_session_view_mode();
    None
}
```

In `src/ui/tables.rs`, add a `source` column and helper:

```rust
fn session_source_label(session: &Session, depth: usize) -> String { ... }
```

Use two spaces per depth before `sub <nickname>` and include short parent id for orphan subagents.

In `src/ui/details.rs`, extend session details to eight fields:

```rust
("source", session.thread_source.clone()),
("parent", session.parent_thread_id.clone().unwrap_or_else(|| "-".to_string())),
("agent", session.agent_nickname.clone().unwrap_or_else(|| "-".to_string())),
("role", session.agent_role.clone().unwrap_or_else(|| "-".to_string())),
("depth", session.agent_depth.map_or_else(|| "-".to_string(), |depth| depth.to_string())),
```

In `src/ui/chrome.rs`, include current view mode in the Sessions title and add `v view` to the footer.

- [ ] **Step 4: Run UI tests and commit**

Run:

```bash
cargo test ui::input::tests ui::tests --all-features --locked
```

Expected: PASS.

Commit:

```bash
git add src/ui/page_input.rs src/ui/tables.rs src/ui/details.rs src/ui/chrome.rs src/ui/input.rs
git commit -m "feat: 展示会话树来源"
```

## Task 4: Documentation and Full Verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

Update Sessions feature bullets to say Sessions default to tree view when parent metadata exists. Add `v` to the Sessions shortcut table:

```markdown
| `v` | 切换树形/平铺视图 |
```

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS. If it fails only due to formatting, run `cargo fmt --all`, then re-run the check.

- [ ] **Step 3: Run full tests**

Run:

```bash
cargo test --all-features --locked
```

Expected: PASS.

- [ ] **Step 4: Check diff hygiene**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 5: Commit docs/final polish**

Commit README and any formatting-only changes:

```bash
git add README.md
git commit -m "docs: 说明会话树视图"
```

If README was already included in a previous commit and there are no changes, skip this commit.

## Self-Review Notes

- Spec coverage: metadata parsing is Task 1; Tree/Flat state is Task 2; input, list, footer, and details are Task 3; README and full verification are Task 4.
- No relation is inferred from message text; only `session_meta` fields are parsed.
- Expand/collapse and parent navigation remain out of scope.
