//! Accessibility and test diagnostics semantic tree.
//!
//! Provides a standardized semantic tree (roles, accessible names, values,
//! bounding geometry, and states) for accessibility screen readers and automated
//! GUI test harnesses with stable widget IDs.

use crate::{CheckState, LogicalRect, WidgetId};

/// Semantic role of a UI component for screen readers and GUI test drivers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    Window,
    Button,
    IconButton,
    Checkbox,
    Switch,
    Tab,
    TabBar,
    TextField,
    PasswordField,
    Dialog,
    Scrollbar,
    Label,
    List,
    ListItem,
}

/// A node in the accessibility and testing hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    pub id: WidgetId,
    pub role: SemanticRole,
    pub label: Option<String>,
    pub value: Option<String>,
    pub bounds: LogicalRect,
    pub enabled: bool,
    pub focused: bool,
    pub checked: Option<CheckState>,
    pub selected: bool,
    pub children: Vec<SemanticNode>,
}

impl SemanticNode {
    pub fn new(id: WidgetId, role: SemanticRole, bounds: LogicalRect) -> Self {
        Self {
            id,
            role,
            label: None,
            value: None,
            bounds,
            enabled: true,
            focused: false,
            checked: None,
            selected: false,
            children: Vec::new(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn with_checked(mut self, checked: Option<CheckState>) -> Self {
        self.checked = checked;
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn add_child(&mut self, child: SemanticNode) {
        self.children.push(child);
    }

    /// Recursively search for a node by its `WidgetId`.
    pub fn find_by_id(&self, id: WidgetId) -> Option<&SemanticNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Recursively search for a node by its accessible label.
    pub fn find_by_label(&self, label: &str) -> Option<&SemanticNode> {
        if self.label.as_deref() == Some(label) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_label(label) {
                return Some(found);
            }
        }
        None
    }

    /// Recursively collect all nodes matching a specific semantic role.
    pub fn find_all_by_role(&self, role: SemanticRole) -> Vec<&SemanticNode> {
        let mut results = Vec::new();
        self.collect_by_role(role, &mut results);
        results
    }

    fn collect_by_role<'a>(&'a self, role: SemanticRole, out: &mut Vec<&'a SemanticNode>) {
        if self.role == role {
            out.push(self);
        }
        for child in &self.children {
            child.collect_by_role(role, out);
        }
    }
}

/// Container for the window's semantic accessibility tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticTree {
    pub root: Option<SemanticNode>,
}

impl SemanticTree {
    pub fn new(root: Option<SemanticNode>) -> Self {
        Self { root }
    }

    pub fn find_by_id(&self, id: WidgetId) -> Option<&SemanticNode> {
        self.root.as_ref().and_then(|r| r.find_by_id(id))
    }

    pub fn find_by_label(&self, label: &str) -> Option<&SemanticNode> {
        self.root.as_ref().and_then(|r| r.find_by_label(label))
    }

    pub fn find_all_by_role(&self, role: SemanticRole) -> Vec<&SemanticNode> {
        self.root
            .as_ref()
            .map_or(Vec::new(), |r| r.find_all_by_role(role))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_tree_construction_and_lookups() {
        let window_bounds = LogicalRect::from_xywh(0.0, 0.0, 800.0, 600.0);
        let mut root = SemanticNode::new(WidgetId(1), SemanticRole::Window, window_bounds);

        let button = SemanticNode::new(
            WidgetId(10),
            SemanticRole::Button,
            LogicalRect::from_xywh(10.0, 20.0, 100.0, 36.0),
        )
        .with_label("Save");

        let checkbox = SemanticNode::new(
            WidgetId(20),
            SemanticRole::Checkbox,
            LogicalRect::from_xywh(10.0, 70.0, 150.0, 24.0),
        )
        .with_label("Auto-save")
        .with_checked(Some(CheckState::Checked));

        root.add_child(button);
        root.add_child(checkbox);

        let tree = SemanticTree::new(Some(root));

        // Find by ID
        let found_button = tree.find_by_id(WidgetId(10)).expect("button must be found");
        assert_eq!(found_button.role, SemanticRole::Button);
        assert_eq!(found_button.label.as_deref(), Some("Save"));

        // Find by Label
        let found_cb = tree
            .find_by_label("Auto-save")
            .expect("checkbox must be found");
        assert_eq!(found_cb.role, SemanticRole::Checkbox);
        assert_eq!(found_cb.checked, Some(CheckState::Checked));

        // Find all by role
        let buttons = tree.find_all_by_role(SemanticRole::Button);
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].id, WidgetId(10));
    }
}
