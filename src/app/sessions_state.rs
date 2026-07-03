use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use ratatui::widgets::TableState;

use crate::session_store::{Session, matches_search, search_terms};

use super::{CurrentDirMatcher, ProviderTabs, Scope, SearchState, TableSelection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionViewMode {
    Tree,
    Flat,
}

pub struct SessionsState {
    pub(super) items: Vec<Session>,
    pub(super) provider_tabs: ProviderTabs,
    pub(super) selection: TableSelection,
    pub(super) visible_indices: Vec<usize>,
    pub(super) visible_depths: Vec<usize>,
    pub(super) visible_tree_prefixes: Vec<String>,
    pub(super) visible_parent_links: Vec<bool>,
    pub(super) view_mode: SessionViewMode,
    pub(super) search: SearchState,
    pub(super) scope: Scope,
    pub(super) current_dir: PathBuf,
    pub(super) sessions_dir: PathBuf,
}

impl SessionsState {
    pub(crate) fn new(items: Vec<Session>, current_dir: PathBuf, sessions_dir: PathBuf) -> Self {
        let provider_tabs = ProviderTabs::new(&items, Scope::CurrentDir, &current_dir);
        Self {
            items,
            provider_tabs,
            selection: TableSelection::default(),
            visible_indices: Vec::new(),
            visible_depths: Vec::new(),
            visible_tree_prefixes: Vec::new(),
            visible_parent_links: Vec::new(),
            view_mode: SessionViewMode::Tree,
            search: SearchState::default(),
            scope: Scope::CurrentDir,
            current_dir,
            sessions_dir,
        }
    }

    pub(crate) fn items(&self) -> &[Session] {
        &self.items
    }

    pub(crate) fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub(crate) fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    pub(crate) const fn scope(&self) -> Scope {
        self.scope
    }

    pub(crate) const fn provider_tabs(&self) -> &ProviderTabs {
        &self.provider_tabs
    }

    pub(crate) const fn move_provider_tab(&mut self, delta: isize) {
        self.provider_tabs.move_by(delta);
    }

    pub(crate) fn selected_provider_label_owned(&self) -> Option<String> {
        self.provider_tabs.selected_label_owned()
    }

    pub(crate) const fn search(&self) -> &SearchState {
        &self.search
    }

    pub(crate) const fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    pub(crate) const fn visible_len(&self) -> usize {
        self.visible_indices.len()
    }

    pub(crate) const fn view_mode(&self) -> SessionViewMode {
        self.view_mode
    }

    pub(crate) const fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            SessionViewMode::Tree => SessionViewMode::Flat,
            SessionViewMode::Flat => SessionViewMode::Tree,
        };
    }

    #[cfg(test)]
    pub(crate) fn visible_depth(&self, visible_index: usize) -> usize {
        self.visible_depths
            .get(visible_index)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn visible_tree_prefix(&self, visible_index: usize) -> &str {
        self.visible_tree_prefixes
            .get(visible_index)
            .map_or("", String::as_str)
    }

    pub(crate) fn visible_parent_link(&self, visible_index: usize) -> bool {
        self.visible_parent_links
            .get(visible_index)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn visible_session(&self, visible_index: usize) -> Option<&Session> {
        self.visible_indices
            .get(visible_index)
            .and_then(|index| self.items.get(*index))
    }

    pub(crate) fn selected_session(&self) -> Option<&Session> {
        self.visible_session(self.selection_index())
    }

    pub(crate) fn refresh_visible(&mut self) {
        self.provider_tabs
            .rebuild(&self.items, self.scope, &self.current_dir);

        let selected_provider = self.provider_tabs.selected_provider().map(str::to_string);
        let query_terms = search_terms(self.search.query());
        let current_dir_matcher = match self.scope {
            Scope::CurrentDir => Some(CurrentDirMatcher::new(&self.current_dir)),
            Scope::All => None,
        };
        let candidates = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                current_dir_matcher
                    .as_ref()
                    .is_none_or(|matcher| matcher.matches(&session.cwd))
            })
            .filter(|(_, session)| {
                selected_provider
                    .as_deref()
                    .is_none_or(|provider| provider == session.provider.as_str())
            })
            .filter(|(_, session)| matches_search(session, &query_terms))
            .map(|(index, _)| index)
            .collect();
        match self.view_mode {
            SessionViewMode::Tree => self.rebuild_tree_visible(candidates),
            SessionViewMode::Flat => self.rebuild_flat_visible(candidates),
        }

        self.selection.sync_len(self.visible_indices.len());
    }

    fn rebuild_flat_visible(&mut self, candidates: Vec<usize>) {
        self.visible_parent_links = self.parent_link_flags(&candidates);
        self.visible_depths = vec![0; candidates.len()];
        self.visible_tree_prefixes = vec![String::new(); candidates.len()];
        self.visible_indices = candidates;
    }

    fn rebuild_tree_visible(&mut self, candidates: Vec<usize>) {
        let candidate_set = candidates.iter().copied().collect::<HashSet<_>>();
        let id_to_index = candidates
            .iter()
            .filter_map(|index| Some((self.items.get(*index)?.id.as_str(), *index)))
            .collect::<HashMap<_, _>>();
        let mut children = HashMap::<usize, Vec<usize>>::new();
        let mut roots = Vec::new();

        for index in candidates {
            let Some(session) = self.items.get(index) else {
                continue;
            };
            if let Some(parent_index) = session
                .parent_thread_id
                .as_deref()
                .and_then(|parent_id| id_to_index.get(parent_id))
                .copied()
                .filter(|parent_index| candidate_set.contains(parent_index))
            {
                children.entry(parent_index).or_default().push(index);
            } else {
                roots.push(index);
            }
        }

        sort_session_indices(&self.items, &mut roots);
        for child_indices in children.values_mut() {
            sort_session_indices(&self.items, child_indices);
        }

        let mut rows = TreeRows::new(&children);
        for root in roots {
            let show_parent_link = self.items[root].parent_thread_id.is_some();
            rows.append(root, 0, "● ".to_string(), show_parent_link, "");
        }

        let mut remaining = candidate_set
            .into_iter()
            .filter(|index| !rows.is_visited(*index))
            .collect::<Vec<_>>();
        sort_session_indices(&self.items, &mut remaining);
        for index in remaining {
            let show_parent_link = self.items[index].parent_thread_id.is_some();
            rows.append(index, 0, "● ".to_string(), show_parent_link, "");
        }

        let (visible_indices, visible_depths, visible_tree_prefixes, visible_parent_links) =
            rows.into_parts();
        self.visible_indices = visible_indices;
        self.visible_depths = visible_depths;
        self.visible_tree_prefixes = visible_tree_prefixes;
        self.visible_parent_links = visible_parent_links;
    }

    fn parent_link_flags(&self, indices: &[usize]) -> Vec<bool> {
        let ids = indices
            .iter()
            .filter_map(|index| Some(self.items.get(*index)?.id.as_str()))
            .collect::<HashSet<_>>();
        indices
            .iter()
            .map(|index| {
                self.items
                    .get(*index)
                    .and_then(|session| session.parent_thread_id.as_deref())
                    .is_some_and(|parent_id| !ids.contains(parent_id))
            })
            .collect()
    }

    pub(crate) fn rebuild_provider_tabs_preserving_label(&mut self, selected_label: Option<&str>) {
        self.provider_tabs.rebuild_preserving_label(
            &self.items,
            self.scope,
            &self.current_dir,
            selected_label,
        );
    }

    pub(crate) fn replace_items(&mut self, items: Vec<Session>) {
        self.items = items;
    }

    pub(crate) fn select_visible_session_by_id(&mut self, session_id: &str) {
        if let Some(index) = self
            .visible_indices
            .iter()
            .position(|session_index| self.items[*session_index].id == session_id)
        {
            self.selection.select(index);
        }
    }

    pub(crate) const fn move_selection(&mut self, delta: isize) {
        self.selection.move_by(self.visible_indices.len(), delta);
    }

    pub(crate) fn page_selection(&mut self, delta: isize) {
        self.selection
            .move_by_clamped(self.visible_indices.len(), delta);
    }

    pub(crate) fn toggle_scope(&mut self) {
        self.scope = match self.scope {
            Scope::CurrentDir => Scope::All,
            Scope::All => Scope::CurrentDir,
        };
        self.selection.reset();
        self.refresh_visible();
    }

    pub(crate) fn clear_search(&mut self) {
        self.search.clear();
        self.selection.reset();
        self.refresh_visible();
    }

    pub(crate) const fn selection_index(&self) -> usize {
        self.selection.index()
    }

    pub(crate) const fn reset_selection(&mut self) {
        self.selection.reset();
    }

    #[cfg(test)]
    pub(crate) const fn select_index(&mut self, index: usize) {
        self.selection.select(index);
    }

    pub(crate) const fn selection_state_mut(&mut self) -> &mut TableState {
        self.selection.state_mut()
    }
}

fn sort_session_indices(items: &[Session], indices: &mut [usize]) {
    indices.sort_by(|left, right| {
        let left_session = &items[*left];
        let right_session = &items[*right];
        right_session
            .timestamp
            .cmp(&left_session.timestamp)
            .then_with(|| left_session.id.cmp(&right_session.id))
    });
}

struct TreeRows<'a> {
    children: &'a HashMap<usize, Vec<usize>>,
    visited: HashSet<usize>,
    visible_indices: Vec<usize>,
    visible_depths: Vec<usize>,
    visible_tree_prefixes: Vec<String>,
    visible_parent_links: Vec<bool>,
}

impl<'a> TreeRows<'a> {
    fn new(children: &'a HashMap<usize, Vec<usize>>) -> Self {
        Self {
            children,
            visited: HashSet::new(),
            visible_indices: Vec::new(),
            visible_depths: Vec::new(),
            visible_tree_prefixes: Vec::new(),
            visible_parent_links: Vec::new(),
        }
    }

    fn is_visited(&self, index: usize) -> bool {
        self.visited.contains(&index)
    }

    fn append(
        &mut self,
        index: usize,
        depth: usize,
        tree_prefix: String,
        show_parent_link: bool,
        child_prefix_base: &str,
    ) {
        if !self.visited.insert(index) {
            return;
        }
        self.visible_indices.push(index);
        self.visible_depths.push(depth);
        self.visible_tree_prefixes.push(tree_prefix);
        self.visible_parent_links.push(show_parent_link);

        if let Some(child_indices) = self.children.get(&index) {
            for (child_position, child_index) in child_indices.iter().enumerate() {
                let is_last = child_position + 1 == child_indices.len();
                let connector = if is_last { "└─ " } else { "├─ " };
                let next_child_prefix_base = if is_last { "   " } else { "│  " };
                let next_child_prefix = format!("{child_prefix_base}{next_child_prefix_base}");
                self.append(
                    *child_index,
                    depth + 1,
                    format!("{child_prefix_base}{connector}"),
                    false,
                    &next_child_prefix,
                );
            }
        }
    }

    fn into_parts(self) -> (Vec<usize>, Vec<usize>, Vec<String>, Vec<bool>) {
        (
            self.visible_indices,
            self.visible_depths,
            self.visible_tree_prefixes,
            self.visible_parent_links,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_session(id: &str, cwd: PathBuf, provider: &str, summary: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: summary.to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

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

    #[cfg(unix)]
    #[test]
    fn refresh_visible_matches_symlinked_current_dir() {
        let dir = tempdir().unwrap();
        let real_project = dir.path().join("real-project");
        let linked_project = dir.path().join("linked-project");
        std::fs::create_dir(&real_project).unwrap();
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();
        let mut state = SessionsState::new(
            vec![test_session("1", real_project, "alpha", "first request")],
            linked_project,
            PathBuf::from("sessions"),
        );

        state.refresh_visible();

        assert_eq!(state.visible_len(), 1);
        assert_eq!(state.selected_session().unwrap().id, "1");
    }

    #[test]
    fn refresh_visible_filters_by_scope_provider_and_search() {
        let current_dir = PathBuf::from("/repo/current");
        let mut state = SessionsState::new(
            vec![
                test_session("1", current_dir.clone(), "alpha", "first request"),
                test_session("2", current_dir.clone(), "beta", "second request"),
                test_session("3", PathBuf::from("/repo/other"), "alpha", "third request"),
            ],
            current_dir,
            PathBuf::from("sessions"),
        );

        state.provider_tabs.set_selected_index(99);
        state.search_mut().set_query("second");
        state.refresh_visible();

        assert_eq!(state.provider_tabs().selected_index(), 0);
        assert_eq!(state.visible_len(), 1);
        assert_eq!(state.selected_session().unwrap().id, "2");
        assert_eq!(state.selection.state().selected(), Some(0));
    }

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

        let mut state =
            SessionsState::new(vec![child, parent], current_dir, PathBuf::from("sessions"));
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

        let mut state =
            SessionsState::new(vec![child, parent], current_dir, PathBuf::from("sessions"));
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

        let mut state =
            SessionsState::new(vec![parent, child], current_dir, PathBuf::from("sessions"));
        state.toggle_view_mode();
        state.refresh_visible();

        assert_eq!(state.view_mode(), SessionViewMode::Flat);
        assert_eq!(state.visible_tree_prefix(0), "");
        assert_eq!(state.visible_tree_prefix(1), "");
    }
}
