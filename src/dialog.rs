//! Modal dialog containers with backdrop isolation, focus trapping, and default/cancel actions.

use crate::{
    CursorIcon, HitRegion, Key, LogicalPoint, LogicalRect, Modifiers, PaintBuilder, PaintLayer,
    Theme, WidgetId,
};

/// Action resulting from dialog keyboard or pointer interaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogAction {
    None,
    Confirm,
    Cancel,
    Custom(u32),
}

/// A modal dialog container with background dimming (backdrop), pointer event isolation,
/// and keyboard shortcuts (Escape to cancel, Enter to confirm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dialog {
    pub id: WidgetId,
    pub backdrop_id: WidgetId,
    pub bounds: LogicalRect,
    pub default_button: Option<WidgetId>,
    pub cancel_button: Option<WidgetId>,
    pub close_on_escape: bool,
    pub close_on_backdrop: bool,
}

impl Dialog {
    pub fn new(id: WidgetId, backdrop_id: WidgetId, bounds: LogicalRect) -> Self {
        Self {
            id,
            backdrop_id,
            bounds,
            default_button: None,
            cancel_button: None,
            close_on_escape: true,
            close_on_backdrop: false,
        }
    }

    pub fn with_buttons(
        mut self,
        default_button: Option<WidgetId>,
        cancel_button: Option<WidgetId>,
    ) -> Self {
        self.default_button = default_button;
        self.cancel_button = cancel_button;
        self
    }

    pub fn with_close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Handles keyboard events for dialog shortcuts.
    /// Escape triggers `Cancel`, Enter triggers `Confirm` (or the default button).
    pub fn handle_key(&self, key: Key, modifiers: Modifiers) -> DialogAction {
        if self.close_on_escape && key == Key::Escape && !modifiers.shift && !modifiers.control && !modifiers.alt {
            return DialogAction::Cancel;
        }

        if key == Key::Enter && !modifiers.shift && !modifiers.control && !modifiers.alt {
            return DialogAction::Confirm;
        }

        DialogAction::None
    }

    /// Handles pointer clicks on the backdrop or dialog area.
    pub fn handle_pointer_down(&self, point: LogicalPoint) -> DialogAction {
        if self.bounds.contains(point) {
            DialogAction::None
        } else if self.close_on_backdrop {
            DialogAction::Cancel
        } else {
            DialogAction::None
        }
    }

    /// Generates the modal backdrop and dialog body hit regions in `PaintLayer::Overlay`.
    /// The backdrop absorbs pointer clicks, isolating underlying background widgets (e.g. terminals or canvases).
    pub fn hit_regions(&self, backdrop_rect: LogicalRect) -> [HitRegion; 2] {
        [
            HitRegion {
                id: self.backdrop_id,
                rect: backdrop_rect,
                layer: PaintLayer::Overlay,
                paint_order: 100,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            },
            HitRegion {
                id: self.id,
                rect: self.bounds,
                layer: PaintLayer::Overlay,
                paint_order: 101,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            },
        ]
    }

    /// Paints the modal backdrop (using `theme.colors.scrim`) and the dialog panel background in `PaintLayer::Overlay`.
    pub fn paint(&self, backdrop_rect: LogicalRect, theme: Theme, builder: &mut PaintBuilder) {
        let colors = theme.colors;
        let metrics = theme.metrics;

        // Dim background
        builder.fill_rect(backdrop_rect, colors.scrim, 0.0);

        // Dialog panel surface
        builder.fill_rect(self.bounds, colors.surface, metrics.radius_lg.0);
        builder.stroke_rect(
            self.bounds,
            colors.border,
            metrics.border_width.0,
            metrics.radius_lg.0,
        );
    }
}

/// Result of standard 3-button confirmation ("Save changes before closing?").
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmChoice {
    Save,
    Discard,
    Cancel,
}

/// Pre-built 3-action confirmation dialog state.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmationDialog {
    pub dialog: Dialog,
    pub title: String,
    pub message: String,
    pub save_button: WidgetId,
    pub discard_button: WidgetId,
    pub cancel_button: WidgetId,
}

impl ConfirmationDialog {
    pub fn new_save_changes(
        dialog_id: WidgetId,
        backdrop_id: WidgetId,
        bounds: LogicalRect,
        save_button: WidgetId,
        discard_button: WidgetId,
        cancel_button: WidgetId,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let dialog = Dialog::new(dialog_id, backdrop_id, bounds)
            .with_buttons(Some(save_button), Some(cancel_button));
        Self {
            dialog,
            title: title.into(),
            message: message.into(),
            save_button,
            discard_button,
            cancel_button,
        }
    }

    /// Resolves clicked button ID into a typed confirmation choice.
    pub fn handle_button_click(&self, target: WidgetId) -> Option<ConfirmChoice> {
        if target == self.save_button {
            Some(ConfirmChoice::Save)
        } else if target == self.discard_button {
            Some(ConfirmChoice::Discard)
        } else if target == self.cancel_button {
            Some(ConfirmChoice::Cancel)
        } else {
            None
        }
    }

    /// Handles keyboard shortcut for the confirmation dialog:
    /// Enter -> Save, Escape -> Cancel.
    pub fn handle_key(&self, key: Key, modifiers: Modifiers) -> Option<ConfirmChoice> {
        match self.dialog.handle_key(key, modifiers) {
            DialogAction::Confirm => Some(ConfirmChoice::Save),
            DialogAction::Cancel => Some(ConfirmChoice::Cancel),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_key_shortcuts_route_confirm_and_cancel() {
        let dialog = Dialog::new(
            WidgetId(10),
            WidgetId(11),
            LogicalRect::from_xywh(100.0, 100.0, 300.0, 200.0),
        );

        assert_eq!(
            dialog.handle_key(Key::Escape, Modifiers::default()),
            DialogAction::Cancel
        );
        assert_eq!(
            dialog.handle_key(Key::Enter, Modifiers::default()),
            DialogAction::Confirm
        );
        assert_eq!(
            dialog.handle_key(Key::Character('a'), Modifiers::default()),
            DialogAction::None
        );
        // Shift+Enter does not confirm
        assert_eq!(
            dialog.handle_key(
                Key::Enter,
                Modifiers {
                    shift: true,
                    ..Default::default()
                }
            ),
            DialogAction::None
        );
    }

    #[test]
    fn backdrop_clicks_isolate_content_and_optionally_dismiss() {
        let dialog = Dialog::new(
            WidgetId(10),
            WidgetId(11),
            LogicalRect::from_xywh(100.0, 100.0, 300.0, 200.0),
        );

        // Inside click never dismisses
        assert_eq!(
            dialog.handle_pointer_down(LogicalPoint::new(150.0, 150.0)),
            DialogAction::None
        );

        // Outside click does not dismiss by default
        assert_eq!(
            dialog.handle_pointer_down(LogicalPoint::new(50.0, 50.0)),
            DialogAction::None
        );

        // With close_on_backdrop enabled:
        let dismissible = dialog.with_close_on_backdrop(true);
        assert_eq!(
            dismissible.handle_pointer_down(LogicalPoint::new(50.0, 50.0)),
            DialogAction::Cancel
        );
        assert_eq!(
            dismissible.handle_pointer_down(LogicalPoint::new(150.0, 150.0)),
            DialogAction::None
        );
    }

    #[test]
    fn dialog_hit_regions_shield_background_in_overlay_layer() {
        let dialog = Dialog::new(
            WidgetId(10),
            WidgetId(11),
            LogicalRect::from_xywh(100.0, 100.0, 300.0, 200.0),
        );
        let backdrop_rect = LogicalRect::from_xywh(0.0, 0.0, 800.0, 600.0);
        let [backdrop_hr, body_hr] = dialog.hit_regions(backdrop_rect);

        assert_eq!(backdrop_hr.layer, PaintLayer::Overlay);
        assert_eq!(body_hr.layer, PaintLayer::Overlay);
        assert_eq!(backdrop_hr.id, WidgetId(11));
        assert_eq!(body_hr.id, WidgetId(10));
        assert!(body_hr.paint_order > backdrop_hr.paint_order);

        // Simulated hit test with a background button at (50, 50)
        let background_button = HitRegion {
            id: WidgetId(1),
            rect: LogicalRect::from_xywh(40.0, 40.0, 100.0, 40.0),
            layer: PaintLayer::Content,
            paint_order: 50,
            enabled: true,
            focusable: true,
            cursor: CursorIcon::Pointer,
        };
        let hit = HitRegion::hit_test(
            LogicalPoint::new(50.0, 50.0),
            &[background_button, backdrop_hr, body_hr],
        );
        assert_eq!(
            hit.map(|r| r.id),
            Some(WidgetId(11)),
            "backdrop must intercept click over background button"
        );
    }

    #[test]
    fn confirmation_dialog_resolves_choices_accurately() {
        let confirm = ConfirmationDialog::new_save_changes(
            WidgetId(10),
            WidgetId(11),
            LogicalRect::from_xywh(100.0, 100.0, 300.0, 200.0),
            WidgetId(20),
            WidgetId(21),
            WidgetId(22),
            "Unsaved Changes",
            "Do you want to save changes before closing?",
        );

        assert_eq!(confirm.handle_button_click(WidgetId(20)), Some(ConfirmChoice::Save));
        assert_eq!(confirm.handle_button_click(WidgetId(21)), Some(ConfirmChoice::Discard));
        assert_eq!(confirm.handle_button_click(WidgetId(22)), Some(ConfirmChoice::Cancel));
        assert_eq!(confirm.handle_button_click(WidgetId(99)), None);

        assert_eq!(confirm.handle_key(Key::Enter, Modifiers::default()), Some(ConfirmChoice::Save));
        assert_eq!(confirm.handle_key(Key::Escape, Modifiers::default()), Some(ConfirmChoice::Cancel));
    }
}
