use super::PaneSession;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct Pane {
    pub id: String,
    pub session_id: String,
    // The selected pane's state is temporarily held by the app for input and rendering.
    pub(super) session: PaneSession,
}

impl Pane {
    pub fn new(id: String, session_id: String) -> Self {
        Self {
            id,
            session_id,
            session: PaneSession::default(),
        }
    }
}

pub enum PaneNode {
    Leaf(Box<Pane>),
    Branch {
        axis: SplitAxis,
        children: Vec<(f32, PaneNode)>,
    },
}

pub struct PaneTree {
    root: PaneNode,
    active_id: String,
    next_id: u32,
}

impl PaneTree {
    pub fn new(initial_pane: Pane) -> Self {
        let id = initial_pane.id.clone();
        Self {
            root: PaneNode::Leaf(Box::new(initial_pane)),
            active_id: id,
            next_id: 1,
        }
    }

    pub fn active_id(&self) -> &str {
        &self.active_id
    }

    pub fn set_active(&mut self, id: &str) {
        if self.find_pane(id).is_some() {
            self.active_id = id.to_string();
        }
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.find_pane(&self.active_id)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.active_id.clone();
        self.find_pane_mut(&id)
    }

    pub fn pane_count(&self) -> usize {
        Self::count_leaves(&self.root)
    }

    pub fn all_pane_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        Self::collect_ids(&self.root, &mut ids);
        ids
    }

    pub fn find_pane(&self, id: &str) -> Option<&Pane> {
        Self::find_in_node(&self.root, id)
    }

    pub fn find_pane_mut(&mut self, id: &str) -> Option<&mut Pane> {
        Self::find_in_node_mut(&mut self.root, id)
    }

    pub fn split(&mut self, pane_id: &str, axis: SplitAxis, session_id: String) -> Option<String> {
        let new_id = format!("pane-{}", self.next_id);
        self.next_id += 1;
        let new_pane = Pane::new(new_id.clone(), session_id);
        let mut pane_slot = Some(new_pane);

        if Self::split_node(&mut self.root, pane_id, axis, &mut pane_slot) {
            self.active_id = new_id.clone();
            Some(new_id)
        } else {
            None
        }
    }

    pub fn close(&mut self, pane_id: &str) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }

        let removed = Self::remove_node(&mut self.root, pane_id);
        if removed {
            if self.active_id == pane_id {
                let ids = self.all_pane_ids();
                if let Some(first) = ids.first() {
                    self.active_id = first.clone();
                }
            }
            Self::simplify_node(&mut self.root);
        }
        removed
    }

    pub fn navigate(&mut self, direction: Direction) {
        let ids = self.all_pane_ids();
        if ids.len() <= 1 {
            return;
        }
        let current_idx = ids.iter().position(|id| id == &self.active_id).unwrap_or(0);
        let next_idx = match direction {
            Direction::Right | Direction::Down => (current_idx + 1) % ids.len(),
            Direction::Left | Direction::Up => {
                if current_idx == 0 {
                    ids.len() - 1
                } else {
                    current_idx - 1
                }
            }
        };
        self.active_id = ids[next_idx].clone();
    }

    pub fn for_each_pane<F: FnMut(&Pane)>(&self, mut f: F) {
        Self::visit_leaves(&self.root, &mut f);
    }

    pub fn for_each_pane_mut<F: FnMut(&mut Pane)>(&mut self, mut f: F) {
        Self::visit_leaves_mut(&mut self.root, &mut f);
    }

    /// Resolve each pane's rectangle using its parent split and share of the available space.
    pub fn layout(&self, rect: egui::Rect) -> Vec<(String, egui::Rect)> {
        let mut panes = Vec::new();
        Self::layout_node(&self.root, rect, &mut panes);
        panes
    }

    fn layout_node(node: &PaneNode, rect: egui::Rect, panes: &mut Vec<(String, egui::Rect)>) {
        match node {
            PaneNode::Leaf(pane) => panes.push((pane.id.clone(), rect)),
            PaneNode::Branch { axis, children } => {
                let total_flex: f32 = children.iter().map(|(flex, _)| *flex).sum();
                let length = match axis {
                    SplitAxis::Horizontal => rect.width(),
                    SplitAxis::Vertical => rect.height(),
                };
                let available = (length - 2.0 * children.len().saturating_sub(1) as f32).max(0.0);
                let mut offset = 0.0;
                for (flex, child) in children {
                    let size = available * flex / total_flex;
                    let mut child_rect = rect;
                    match axis {
                        SplitAxis::Horizontal => {
                            child_rect.min.x += offset;
                            child_rect.max.x = child_rect.min.x + size;
                        }
                        SplitAxis::Vertical => {
                            child_rect.min.y += offset;
                            child_rect.max.y = child_rect.min.y + size;
                        }
                    }
                    Self::layout_node(child, child_rect, panes);
                    offset += size + 2.0;
                }
            }
        }
    }

    fn find_in_node<'a>(node: &'a PaneNode, id: &str) -> Option<&'a Pane> {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Branch { children, .. } => children
                .iter()
                .find_map(|(_, child)| Self::find_in_node(child, id)),
        }
    }

    fn find_in_node_mut<'a>(node: &'a mut PaneNode, id: &str) -> Option<&'a mut Pane> {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Branch { children, .. } => children
                .iter_mut()
                .find_map(|(_, child)| Self::find_in_node_mut(child, id)),
        }
    }

    fn split_node(
        node: &mut PaneNode,
        target_id: &str,
        axis: SplitAxis,
        new_pane: &mut Option<Pane>,
    ) -> bool {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    if let Some(np) = new_pane.take() {
                        let old_node = std::mem::replace(
                            node,
                            PaneNode::Leaf(Box::new(Pane::new("temp".to_string(), String::new()))),
                        );
                        *node = PaneNode::Branch {
                            axis,
                            children: vec![(1.0, old_node), (1.0, PaneNode::Leaf(Box::new(np)))],
                        };
                        return true;
                    }
                }
                false
            }
            PaneNode::Branch { children, .. } => {
                for (_, child) in children.iter_mut() {
                    if Self::split_node(child, target_id, axis, new_pane) {
                        return true;
                    }
                }
                false
            }
        }
    }

    fn remove_node(node: &mut PaneNode, target_id: &str) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Branch { children, .. } => {
                let idx = children
                    .iter()
                    .position(|(_, child)| matches!(child, PaneNode::Leaf(p) if p.id == target_id));
                if let Some(idx) = idx {
                    children.remove(idx);
                    return true;
                }
                children
                    .iter_mut()
                    .any(|(_, child)| Self::remove_node(child, target_id))
            }
        }
    }

    fn simplify_node(node: &mut PaneNode) {
        if let PaneNode::Branch { children, .. } = node {
            for (_, child) in children.iter_mut() {
                Self::simplify_node(child);
            }
            if children.len() == 1 {
                let (_, only_child) = children.remove(0);
                *node = only_child;
            }
        }
    }

    fn count_leaves(node: &PaneNode) -> usize {
        match node {
            PaneNode::Leaf(_) => 1,
            PaneNode::Branch { children, .. } => {
                children.iter().map(|(_, c)| Self::count_leaves(c)).sum()
            }
        }
    }

    fn collect_ids(node: &PaneNode, ids: &mut Vec<String>) {
        match node {
            PaneNode::Leaf(pane) => ids.push(pane.id.clone()),
            PaneNode::Branch { children, .. } => {
                for (_, child) in children {
                    Self::collect_ids(child, ids);
                }
            }
        }
    }

    fn visit_leaves<F: FnMut(&Pane)>(node: &PaneNode, f: &mut F) {
        match node {
            PaneNode::Leaf(pane) => f(pane),
            PaneNode::Branch { children, .. } => {
                for (_, child) in children {
                    Self::visit_leaves(child, f);
                }
            }
        }
    }

    fn visit_leaves_mut<F: FnMut(&mut Pane)>(node: &mut PaneNode, f: &mut F) {
        match node {
            PaneNode::Leaf(pane) => f(pane),
            PaneNode::Branch { children, .. } => {
                for (_, child) in children {
                    Self::visit_leaves_mut(child, f);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pane(id: &str) -> Pane {
        Pane::new(id.to_string(), String::new())
    }

    #[test]
    fn test_new_tree() {
        let tree = PaneTree::new(make_pane("root"));
        assert_eq!(tree.pane_count(), 1);
        assert_eq!(tree.active_id(), "root");
    }

    #[test]
    fn test_split() {
        let mut tree = PaneTree::new(make_pane("root"));
        let new_id = tree.split("root", SplitAxis::Horizontal, String::new());
        assert!(new_id.is_some());
        assert_eq!(tree.pane_count(), 2);
    }

    #[test]
    fn test_close() {
        let mut tree = PaneTree::new(make_pane("root"));
        let new_id = tree
            .split("root", SplitAxis::Horizontal, String::new())
            .unwrap();
        assert!(tree.close(&new_id));
        assert_eq!(tree.pane_count(), 1);
    }

    #[test]
    fn test_cannot_close_last() {
        let mut tree = PaneTree::new(make_pane("root"));
        assert!(!tree.close("root"));
    }

    #[test]
    fn test_navigate() {
        let mut tree = PaneTree::new(make_pane("root"));
        tree.split("root", SplitAxis::Horizontal, String::new());
        let ids = tree.all_pane_ids();
        tree.set_active(&ids[0]);
        tree.navigate(Direction::Right);
        assert_eq!(tree.active_id(), ids[1]);
    }
}
