//! First reusable controls. They produce layout, hit regions, and paint from
//! the same identity and geometry; application commands remain semantic events.

use std::sync::Arc;

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Color, HitRegion, IconId, InputEvent, InputState, Key, LayoutId, LayoutNode, LogicalPoint,
    LogicalPx, LogicalRect, LogicalSize, Modifiers, PaintBuilder, PaintLayer, TextLayout, TextStyle,
    TextSystem, Theme, WidgetId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalTextAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub id: WidgetId,
    pub text: String,
    pub align: HorizontalTextAlign,
    pub max_width: Option<LogicalPx>,
}

impl Label {
    pub fn new(id: WidgetId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            align: HorizontalTextAlign::Start,
            max_width: None,
        }
    }

    pub fn with_max_width(mut self, width: LogicalPx) -> Self {
        self.max_width = Some(LogicalPx(width.0.max(0.0)));
        self
    }

    pub fn measure(&self, text_system: &mut TextSystem, theme: Theme) -> Arc<TextLayout> {
        let style = text_style(theme);
        if let Some(max_width) = self.max_width {
            let full = text_system.shape(&self.text, &style, None);
            if full.metrics.size.width.0 > max_width.0 {
                let shortened = ellipsized_text(&self.text, max_width, &style, text_system);
                return Arc::new(text_system.shape(&shortened, &style, None));
            }
            return Arc::new(full);
        }
        Arc::new(text_system.shape(&self.text, &style, None))
    }

    pub fn paint(
        &self,
        rect: LogicalRect,
        text_system: &mut TextSystem,
        theme: Theme,
        builder: &mut PaintBuilder,
    ) {
        let layout = self.measure(text_system, theme);
        let x = match self.align {
            HorizontalTextAlign::Start => rect.left(),
            HorizontalTextAlign::Center => {
                rect.left() + (rect.size.width.0 - layout.metrics.size.width.0) / 2.0
            }
            HorizontalTextAlign::End => rect.right() - layout.metrics.size.width.0,
        };
        let y = rect.top() + (rect.size.height.0 - layout.metrics.size.height.0).max(0.0) / 2.0;
        builder.text(LogicalPoint::new(x, y), layout, theme.colors.text_primary);
    }

    pub fn layout_node(&self, measured: &TextLayout) -> LayoutNode {
        LayoutNode::intrinsic(LayoutId(self.id.0), measured.metrics.size)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ButtonState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetEvent {
    Activated(WidgetId),
}

/// Visual emphasis of a button; behaviour is identical.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Default,
    /// The suggested action: filled with the accent colour.
    Primary,
    /// A destructive action: filled with the danger colour.
    Danger,
    /// Borderless and transparent until hovered or pressed (toolbars).
    Flat,
}

#[derive(Clone, Debug)]
pub struct Button {
    pub id: WidgetId,
    pub label: Option<String>,
    pub icon: Option<IconId>,
    pub enabled: bool,
    pub variant: ButtonVariant,
    /// Optional state colour for the icon (e.g. "off" in danger red); the label
    /// and the default icon colour come from the theme.
    pub icon_tint: Option<crate::Color>,
    icon_size: LogicalPx,
    armed_pointer: bool,
    armed_keyboard: bool,
}

#[derive(Clone, Debug)]
pub struct IconButton(Button);

impl IconButton {
    pub fn new(id: WidgetId, icon: IconId) -> Self {
        Self(Button::icon_only(id, icon))
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.0.enabled = enabled;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.0.variant = variant;
        self
    }

    /// Tint the icon with a state colour (`None` restores the theme colour).
    pub fn set_icon_tint(&mut self, tint: Option<Color>) {
        self.0.icon_tint = tint;
    }

    pub fn set_variant(&mut self, variant: ButtonVariant) {
        self.0.variant = variant;
    }

    /// Set the optical icon box in logical pixels. Layout and painting use
    /// the same value so DPI conversion remains owned by the renderer.
    pub fn with_icon_size(mut self, size: LogicalPx) -> Self {
        self.0 = self.0.with_icon_size(size);
        self
    }

    pub fn state(&self, input: &InputState) -> ButtonState {
        self.0.state(input)
    }

    pub fn layout_node(&self, text_system: &mut TextSystem, theme: Theme) -> LayoutNode {
        self.0.layout_node(text_system, theme)
    }

    pub fn hit_region(&self, rect: LogicalRect, layer: PaintLayer, paint_order: u64) -> HitRegion {
        self.0.hit_region(rect, layer, paint_order)
    }

    pub fn handle(
        &mut self,
        target: Option<WidgetId>,
        event: &InputEvent,
        input: &mut InputState,
        rect: LogicalRect,
    ) -> Option<WidgetEvent> {
        self.0.handle(target, event, input, rect)
    }

    pub fn paint(
        &self,
        rect: LogicalRect,
        state: ButtonState,
        text_system: &mut TextSystem,
        theme: Theme,
        builder: &mut PaintBuilder,
    ) {
        self.0.paint(rect, state, text_system, theme, builder);
    }
}

impl Button {
    pub fn new(id: WidgetId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: Some(label.into()),
            icon: None,
            enabled: true,
            variant: ButtonVariant::Default,
            icon_tint: None,
            icon_size: LogicalPx(0.0),
            armed_pointer: false,
            armed_keyboard: false,
        }
    }

    pub fn icon_only(id: WidgetId, icon: IconId) -> Self {
        Self {
            id,
            label: None,
            icon: Some(icon),
            enabled: true,
            variant: ButtonVariant::Default,
            icon_tint: None,
            icon_size: LogicalPx(16.0),
            armed_pointer: false,
            armed_keyboard: false,
        }
    }

    pub fn with_icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        if self.icon_size.0 <= 0.0 {
            self.icon_size = LogicalPx(16.0);
        }
        self
    }

    /// Set an icon's optical box in logical pixels. The same metric is used
    /// for intrinsic layout and raster placement.
    pub fn with_icon_size(mut self, size: LogicalPx) -> Self {
        self.icon_size = LogicalPx(if size.0.is_finite() {
            size.0.max(0.0)
        } else {
            0.0
        });
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn state(&self, input: &InputState) -> ButtonState {
        ButtonState {
            hovered: input.hovered == Some(self.id),
            pressed: self.armed_pointer || self.armed_keyboard,
            focused: input.focused() == Some(self.id),
            enabled: self.enabled,
        }
    }

    pub fn hit_region(&self, rect: LogicalRect, layer: PaintLayer, paint_order: u64) -> HitRegion {
        HitRegion {
            id: self.id,
            rect,
            layer,
            paint_order,
            enabled: self.enabled,
            focusable: true,
            cursor: if self.enabled {
                crate::CursorIcon::Pointer
            } else {
                crate::CursorIcon::Default
            },
        }
    }

    pub fn layout_node(&self, text_system: &mut TextSystem, theme: Theme) -> LayoutNode {
        let label_layout = self
            .label
            .as_ref()
            .map(|label| text_system.shape(label, &text_style(theme), None));
        let pad = theme.metrics.space_md.0;
        let icon_size = if self.icon.is_some() {
            self.icon_size.0
        } else {
            0.0
        };
        let gap = if icon_size > 0.0 && label_layout.is_some() {
            theme.metrics.space_sm.0
        } else {
            0.0
        };
        let width = label_layout
            .as_ref()
            .map_or(0.0, |layout| layout.metrics.size.width.0)
            + icon_size
            + gap
            + pad * 2.0;
        let content_height = label_layout.as_ref().map_or(icon_size, |layout| {
            layout.metrics.size.height.0.max(icon_size)
        });
        let height = theme
            .metrics
            .control_height_md
            .0
            .max(content_height + theme.metrics.space_sm.0 * 2.0);
        LayoutNode::intrinsic(
            LayoutId(self.id.0),
            LogicalSize::new(width.max(height * 0.8), height),
        )
        .with_min(LogicalSize::new(0.0, theme.metrics.control_height_md.0))
    }

    pub fn handle(
        &mut self,
        target: Option<WidgetId>,
        event: &InputEvent,
        input: &mut InputState,
        rect: LogicalRect,
    ) -> Option<WidgetEvent> {
        if !self.enabled {
            self.armed_pointer = false;
            self.armed_keyboard = false;
            return None;
        }
        match event {
            InputEvent::PointerDown(crate::MouseButton::Primary) if target == Some(self.id) => {
                self.armed_pointer = true;
                input.capture_pointer(self.id);
                None
            }
            InputEvent::PointerUp(crate::MouseButton::Primary) if self.armed_pointer => {
                self.armed_pointer = false;
                input.release_pointer(self.id);
                if rect.contains(input.pointer) && target == Some(self.id) {
                    Some(WidgetEvent::Activated(self.id))
                } else {
                    None
                }
            }
            InputEvent::Key {
                key: Key::Enter | Key::Space,
                pressed,
                ..
            } if target == Some(self.id) => {
                if *pressed {
                    self.armed_keyboard = true;
                    None
                } else if self.armed_keyboard {
                    self.armed_keyboard = false;
                    Some(WidgetEvent::Activated(self.id))
                } else {
                    None
                }
            }
            // A key event for another target (focus moved while the key was
            // down) must not leave this button armed for a later key-up.
            InputEvent::Key { .. } => {
                self.armed_keyboard = false;
                None
            }
            InputEvent::PointerCancel | InputEvent::WindowFocused(false) => {
                self.armed_pointer = false;
                if matches!(event, InputEvent::WindowFocused(false)) {
                    self.armed_keyboard = false;
                }
                input.release_pointer(self.id);
                None
            }
            _ => None,
        }
    }

    pub fn paint(
        &self,
        rect: LogicalRect,
        state: ButtonState,
        text_system: &mut TextSystem,
        theme: Theme,
        builder: &mut PaintBuilder,
    ) {
        let colors = theme.colors;
        let (fill, border, foreground) = match self.variant {
            ButtonVariant::Default => (colors.surface, colors.border, colors.text_primary),
            ButtonVariant::Primary => (colors.accent, colors.accent, colors.text_on_accent),
            ButtonVariant::Danger => (colors.danger, colors.danger, colors.text_on_accent),
            ButtonVariant::Flat => (
                Color::rgba(0, 0, 0, 0),
                Color::rgba(0, 0, 0, 0),
                colors.text_primary,
            ),
        };
        let background = if !state.enabled {
            match self.variant {
                ButtonVariant::Default => colors.surface,
                ButtonVariant::Flat => fill,
                _ => fill.mix(colors.surface, 0.6),
            }
        } else {
            match (self.variant, state.pressed, state.hovered) {
                (ButtonVariant::Default | ButtonVariant::Flat, true, _) => colors.surface_pressed,
                (ButtonVariant::Default | ButtonVariant::Flat, false, true) => colors.surface_hover,
                (ButtonVariant::Default | ButtonVariant::Flat, false, false) => fill,
                (_, true, _) => fill.mix(Color::rgba(0, 0, 0, 255), 0.22),
                (_, false, true) => fill.mix(Color::rgba(0, 0, 0, 255), 0.10),
                (_, false, false) => fill,
            }
        };
        let foreground = if state.enabled {
            foreground
        } else {
            colors.text_disabled
        };
        builder.fill_rect(rect, background, theme.metrics.radius_sm.0);
        builder.stroke_rect(
            rect,
            border,
            theme.metrics.border_width.0,
            theme.metrics.radius_sm.0,
        );
        if state.focused {
            builder.stroke_rect(
                rect,
                colors.focus,
                theme.metrics.focus_width.0,
                theme.metrics.radius_sm.0,
            );
        }

        let label_layout = self
            .label
            .as_ref()
            .map(|label| Arc::new(text_system.shape(label, &text_style(theme), None)));
        let icon_size = if self.icon.is_some() {
            self.icon_size.0
        } else {
            0.0
        };
        let gap = if icon_size > 0.0 && label_layout.is_some() {
            theme.metrics.space_sm.0
        } else {
            0.0
        };
        let label_width = label_layout
            .as_ref()
            .map_or(0.0, |layout| layout.metrics.size.width.0);
        let group_width = icon_size + gap + label_width;
        let start_x = rect.left() + (rect.size.width.0 - group_width).max(0.0) / 2.0;
        if let Some(icon) = self.icon {
            let icon_rect = LogicalRect::from_xywh(
                start_x,
                rect.top() + (rect.size.height.0 - icon_size) / 2.0,
                icon_size,
                icon_size,
            );
            builder.icon(
                icon,
                icon_rect,
                match (state.enabled, self.icon_tint) {
                    (false, _) => colors.text_disabled,
                    (true, Some(tint)) => tint,
                    (true, None) => foreground,
                },
            );
        }
        if let Some(layout) = label_layout {
            let text_color = foreground;
            let x = start_x + icon_size + gap;
            let y = rect.top() + (rect.size.height.0 - layout.metrics.size.height.0) / 2.0;
            builder.text(LogicalPoint::new(x, y), layout, text_color);
        }
    }
}

fn text_style(theme: Theme) -> TextStyle {
    TextStyle {
        font_size: theme.metrics.font_md,
        line_height: theme.metrics.line_height_md,
        ..TextStyle::default()
    }
}

fn ellipsized_text(
    text: &str,
    max_width: LogicalPx,
    style: &TextStyle,
    system: &mut TextSystem,
) -> String {
    const ELLIPSIS: &str = "…";
    if system.shape(ELLIPSIS, style, None).metrics.size.width.0 > max_width.0 {
        return String::new();
    }
    // Cut only at extended grapheme cluster boundaries so a combining mark,
    // ZWJ sequence or flag is never split from its base by the ellipsis.
    let boundaries: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain([text.len()])
        .collect();
    let mut low = 0usize;
    let mut high = boundaries.len();
    while low < high {
        let middle = (low + high) / 2;
        let candidate = format!("{}{}", &text[..boundaries[middle]], ELLIPSIS);
        if system.shape(&candidate, style, None).metrics.size.width.0 <= max_width.0 {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    let count = low.saturating_sub(1);
    format!("{}{}", &text[..boundaries[count]], ELLIPSIS)
}

/// State of a checkbox: unchecked, checked, or mixed (indeterminate).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

impl CheckState {
    pub fn is_checked(&self) -> bool {
        matches!(self, CheckState::Checked)
    }

    pub fn toggle(&self) -> Self {
        match self {
            CheckState::Unchecked => CheckState::Checked,
            CheckState::Checked | CheckState::Indeterminate => CheckState::Unchecked,
        }
    }
}

/// Standard checkbox with label, focus ring, and check/indeterminate states.
#[derive(Clone, Debug)]
pub struct Checkbox {
    pub id: WidgetId,
    pub label: Option<String>,
    pub state: CheckState,
    pub enabled: bool,
    armed_pointer: bool,
    armed_keyboard: bool,
}

impl Checkbox {
    pub fn new(id: WidgetId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: Some(label.into()),
            state: CheckState::Unchecked,
            enabled: true,
            armed_pointer: false,
            armed_keyboard: false,
        }
    }

    pub fn without_label(id: WidgetId) -> Self {
        Self {
            id,
            label: None,
            state: CheckState::Unchecked,
            enabled: true,
            armed_pointer: false,
            armed_keyboard: false,
        }
    }

    pub fn with_state(mut self, state: CheckState) -> Self {
        self.state = state;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn hit_region(&self, rect: LogicalRect, layer: PaintLayer, paint_order: u64) -> HitRegion {
        HitRegion {
            id: self.id,
            rect,
            layer,
            paint_order,
            enabled: self.enabled,
            focusable: true,
            cursor: if self.enabled {
                crate::CursorIcon::Pointer
            } else {
                crate::CursorIcon::Default
            },
        }
    }

    pub fn layout_node(&self, text_system: &mut TextSystem, theme: Theme) -> LayoutNode {
        let box_size = 18.0;
        let label_layout = self
            .label
            .as_ref()
            .map(|label| text_system.shape(label, &text_style(theme), None));
        let gap = if label_layout.is_some() {
            theme.metrics.space_sm.0
        } else {
            0.0
        };
        let label_width = label_layout
            .as_ref()
            .map_or(0.0, |layout| layout.metrics.size.width.0);
        let height = theme.metrics.control_height_sm.0.max(box_size);
        LayoutNode::intrinsic(
            LayoutId(self.id.0),
            LogicalSize::new(box_size + gap + label_width, height),
        )
    }

    pub fn handle(
        &mut self,
        target: Option<WidgetId>,
        event: &InputEvent,
        input: &mut InputState,
        rect: LogicalRect,
    ) -> Option<CheckState> {
        if !self.enabled {
            self.armed_pointer = false;
            self.armed_keyboard = false;
            return None;
        }
        match event {
            InputEvent::PointerDown(crate::MouseButton::Primary) if target == Some(self.id) => {
                self.armed_pointer = true;
                input.capture_pointer(self.id);
                None
            }
            InputEvent::PointerUp(crate::MouseButton::Primary) if self.armed_pointer => {
                self.armed_pointer = false;
                input.release_pointer(self.id);
                if rect.contains(input.pointer) && target == Some(self.id) {
                    self.state = self.state.toggle();
                    Some(self.state)
                } else {
                    None
                }
            }
            InputEvent::Key {
                key: Key::Space,
                pressed,
                ..
            } if target == Some(self.id) => {
                if *pressed {
                    self.armed_keyboard = true;
                    None
                } else if self.armed_keyboard {
                    self.armed_keyboard = false;
                    self.state = self.state.toggle();
                    Some(self.state)
                } else {
                    None
                }
            }
            InputEvent::Key { .. } => {
                self.armed_keyboard = false;
                None
            }
            InputEvent::PointerCancel | InputEvent::WindowFocused(false) => {
                self.armed_pointer = false;
                self.armed_keyboard = false;
                input.release_pointer(self.id);
                None
            }
            _ => None,
        }
    }

    pub fn paint(
        &self,
        rect: LogicalRect,
        text_system: &mut TextSystem,
        theme: Theme,
        input: &InputState,
        builder: &mut PaintBuilder,
    ) {
        let colors = theme.colors;
        let box_size = 18.0;
        let box_top = rect.top() + (rect.size.height.0 - box_size) / 2.0;
        let box_rect = LogicalRect::from_xywh(rect.left(), box_top, box_size, box_size);

        let focused = input.focused() == Some(self.id);
        let hovered = input.hovered == Some(self.id) && self.enabled;

        let (fill_color, border_color) = match (self.state, self.enabled) {
            (CheckState::Checked | CheckState::Indeterminate, true) => {
                if hovered {
                    (colors.accent.mix(colors.surface_hover, 0.1), colors.accent)
                } else {
                    (colors.accent, colors.accent)
                }
            }
            (CheckState::Checked | CheckState::Indeterminate, false) => {
                (colors.accent.mix(colors.surface, 0.5), colors.border)
            }
            (CheckState::Unchecked, true) => {
                if hovered {
                    (colors.surface_hover, colors.focus)
                } else {
                    (colors.surface, colors.border)
                }
            }
            (CheckState::Unchecked, false) => (colors.surface, colors.border),
        };

        builder.fill_rect(box_rect, fill_color, 4.0);
        builder.stroke_rect(box_rect, border_color, theme.metrics.border_width.0, 4.0);

        if focused {
            builder.stroke_rect(
                box_rect.inset(crate::Insets::all(-2.0)),
                colors.focus,
                theme.metrics.focus_width.0,
                6.0,
            );
        }

        match self.state {
            CheckState::Checked => {
                let mark_color = if self.enabled {
                    colors.text_on_accent
                } else {
                    colors.text_disabled
                };
                let p1 = LogicalPoint::new(box_rect.left() + 4.0, box_rect.top() + 9.5);
                let p2 = LogicalPoint::new(box_rect.left() + 7.5, box_rect.top() + 13.0);
                let p3 = LogicalPoint::new(box_rect.left() + 14.0, box_rect.top() + 5.5);
                builder.stroke_path(vec![p1, p2, p3], mark_color, 2.0);
            }
            CheckState::Indeterminate => {
                let mark_color = if self.enabled {
                    colors.text_on_accent
                } else {
                    colors.text_disabled
                };
                let bar = LogicalRect::from_xywh(
                    box_rect.left() + 4.0,
                    box_rect.top() + (box_size - 2.5) / 2.0,
                    box_size - 8.0,
                    2.5,
                );
                builder.fill_rect(bar, mark_color, 1.0);
            }
            CheckState::Unchecked => {}
        }

        if let Some(ref text) = self.label {
            let label_layout = Arc::new(text_system.shape(text, &text_style(theme), None));
            let label_x = box_rect.right() + theme.metrics.space_sm.0;
            let label_y =
                rect.top() + (rect.size.height.0 - label_layout.metrics.size.height.0) / 2.0;
            let text_color = if self.enabled {
                colors.text_primary
            } else {
                colors.text_disabled
            };
            builder.text(LogicalPoint::new(label_x, label_y), label_layout, text_color);
        }
    }
}

/// A toggle switch (on/off pill switch).
#[derive(Clone, Debug)]
pub struct Switch {
    pub id: WidgetId,
    pub label: Option<String>,
    pub on: bool,
    pub enabled: bool,
    armed_pointer: bool,
    armed_keyboard: bool,
}

impl Switch {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            label: None,
            on: false,
            enabled: true,
            armed_pointer: false,
            armed_keyboard: false,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn hit_region(&self, rect: LogicalRect, layer: PaintLayer, paint_order: u64) -> HitRegion {
        HitRegion {
            id: self.id,
            rect,
            layer,
            paint_order,
            enabled: self.enabled,
            focusable: true,
            cursor: if self.enabled {
                crate::CursorIcon::Pointer
            } else {
                crate::CursorIcon::Default
            },
        }
    }

    pub fn layout_node(&self, text_system: &mut TextSystem, theme: Theme) -> LayoutNode {
        let track_width = 38.0;
        let track_height = 22.0;
        let label_layout = self
            .label
            .as_ref()
            .map(|label| text_system.shape(label, &text_style(theme), None));
        let gap = if label_layout.is_some() {
            theme.metrics.space_sm.0
        } else {
            0.0
        };
        let label_width = label_layout
            .as_ref()
            .map_or(0.0, |layout| layout.metrics.size.width.0);
        let height = theme.metrics.control_height_sm.0.max(track_height);
        LayoutNode::intrinsic(
            LayoutId(self.id.0),
            LogicalSize::new(track_width + gap + label_width, height),
        )
    }

    pub fn handle(
        &mut self,
        target: Option<WidgetId>,
        event: &InputEvent,
        input: &mut InputState,
        rect: LogicalRect,
    ) -> Option<bool> {
        if !self.enabled {
            self.armed_pointer = false;
            self.armed_keyboard = false;
            return None;
        }
        match event {
            InputEvent::PointerDown(crate::MouseButton::Primary) if target == Some(self.id) => {
                self.armed_pointer = true;
                input.capture_pointer(self.id);
                None
            }
            InputEvent::PointerUp(crate::MouseButton::Primary) if self.armed_pointer => {
                self.armed_pointer = false;
                input.release_pointer(self.id);
                if rect.contains(input.pointer) && target == Some(self.id) {
                    self.on = !self.on;
                    Some(self.on)
                } else {
                    None
                }
            }
            InputEvent::Key {
                key: Key::Space | Key::Enter,
                pressed,
                ..
            } if target == Some(self.id) => {
                if *pressed {
                    self.armed_keyboard = true;
                    None
                } else if self.armed_keyboard {
                    self.armed_keyboard = false;
                    self.on = !self.on;
                    Some(self.on)
                } else {
                    None
                }
            }
            InputEvent::Key { .. } => {
                self.armed_keyboard = false;
                None
            }
            InputEvent::PointerCancel | InputEvent::WindowFocused(false) => {
                self.armed_pointer = false;
                self.armed_keyboard = false;
                input.release_pointer(self.id);
                None
            }
            _ => None,
        }
    }

    pub fn paint(
        &self,
        rect: LogicalRect,
        text_system: &mut TextSystem,
        theme: Theme,
        input: &InputState,
        builder: &mut PaintBuilder,
    ) {
        let colors = theme.colors;
        let track_width = 38.0;
        let track_height = 22.0;
        let track_top = rect.top() + (rect.size.height.0 - track_height) / 2.0;
        let track_rect = LogicalRect::from_xywh(rect.left(), track_top, track_width, track_height);

        let focused = input.focused() == Some(self.id);
        let track_color = match (self.on, self.enabled) {
            (true, true) => colors.accent,
            (true, false) => colors.accent.mix(colors.surface, 0.5),
            (false, true) => colors.border,
            (false, false) => colors.border.mix(colors.surface, 0.5),
        };

        builder.fill_rect(track_rect, track_color, track_height / 2.0);

        if focused {
            builder.stroke_rect(
                track_rect.inset(crate::Insets::all(-2.0)),
                colors.focus,
                theme.metrics.focus_width.0,
                (track_height + 4.0) / 2.0,
            );
        }

        let thumb_diameter = 16.0;
        let thumb_pad = 3.0;
        let thumb_x = if self.on {
            track_rect.right() - thumb_pad - thumb_diameter
        } else {
            track_rect.left() + thumb_pad
        };
        let thumb_y = track_rect.top() + thumb_pad;
        let thumb_rect = LogicalRect::from_xywh(thumb_x, thumb_y, thumb_diameter, thumb_diameter);
        builder.fill_rect(thumb_rect, colors.surface, thumb_diameter / 2.0);

        if let Some(ref text) = self.label {
            let label_layout = Arc::new(text_system.shape(text, &text_style(theme), None));
            let label_x = track_rect.right() + theme.metrics.space_sm.0;
            let label_y =
                rect.top() + (rect.size.height.0 - label_layout.metrics.size.height.0) / 2.0;
            let text_color = if self.enabled {
                colors.text_primary
            } else {
                colors.text_disabled
            };
            builder.text(LogicalPoint::new(label_x, label_y), label_layout, text_color);
        }
    }
}

/// A tab in a tab bar.
#[derive(Clone, Debug, PartialEq)]
pub struct TabItem {
    pub id: WidgetId,
    pub close_id: Option<WidgetId>,
    pub title: String,
    pub closable: bool,
    pub disabled: bool,
}

impl TabItem {
    pub fn new(id: WidgetId, title: impl Into<String>) -> Self {
        Self {
            id,
            close_id: None,
            title: title.into(),
            closable: false,
            disabled: false,
        }
    }

    pub fn closable(mut self, close_id: WidgetId) -> Self {
        self.close_id = Some(close_id);
        self.closable = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Events resulting from interacting with a tab bar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabEvent {
    Selected(WidgetId),
    Closed(WidgetId),
}

/// Tab bar managing a collection of tabs, active tab selection, and close actions.
#[derive(Clone, Debug, PartialEq)]
pub struct TabBar {
    pub id: WidgetId,
    pub tabs: Vec<TabItem>,
    pub active_tab: Option<WidgetId>,
    pub scroll_offset: f64,
}

impl TabBar {
    pub fn new(id: WidgetId, tabs: Vec<TabItem>, active_tab: Option<WidgetId>) -> Self {
        Self {
            id,
            tabs,
            active_tab,
            scroll_offset: 0.0,
        }
    }

    pub fn select(&mut self, tab_id: WidgetId) -> bool {
        if self.tabs.iter().any(|t| t.id == tab_id && !t.disabled) {
            self.active_tab = Some(tab_id);
            true
        } else {
            false
        }
    }

    pub fn close(&mut self, tab_id: WidgetId) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(pos);
            if self.active_tab == Some(tab_id) {
                self.active_tab = self.tabs.get(pos).or_else(|| self.tabs.last()).map(|t| t.id);
            }
            true
        } else {
            false
        }
    }

    /// Handles keyboard arrow navigation between tabs.
    pub fn handle_key(&mut self, key: Key, _modifiers: Modifiers) -> Option<TabEvent> {
        if self.tabs.is_empty() {
            return None;
        }
        let current_index = self
            .active_tab
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);

        match key {
            Key::ArrowLeft => {
                let next = if current_index == 0 {
                    self.tabs.len() - 1
                } else {
                    current_index - 1
                };
                let id = self.tabs[next].id;
                self.select(id);
                Some(TabEvent::Selected(id))
            }
            Key::ArrowRight => {
                let next = (current_index + 1) % self.tabs.len();
                let id = self.tabs[next].id;
                self.select(id);
                Some(TabEvent::Selected(id))
            }
            _ => None,
        }
    }

    /// Handles pointer down: distinguishes clicking a tab vs clicking its close button.
    pub fn handle_pointer_down(&mut self, target: WidgetId) -> Option<TabEvent> {
        let hit_close = self
            .tabs
            .iter()
            .find(|t| !t.disabled && t.close_id == Some(target))
            .map(|t| t.id);
        if let Some(id) = hit_close {
            self.close(id);
            return Some(TabEvent::Closed(id));
        }

        let hit_tab = self
            .tabs
            .iter()
            .find(|t| !t.disabled && t.id == target)
            .map(|t| t.id);
        if let Some(id) = hit_tab {
            self.select(id);
            return Some(TabEvent::Selected(id));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MouseButton, PaintOp};

    #[test]
    fn button_uses_layout_geometry_and_produces_semantic_click() {
        let theme = Theme::EVERTY_LIGHT;
        let mut text = TextSystem::with_cache_limits(128, 1024 * 1024);
        let mut button = Button::new(WidgetId(1), "Открыть").with_icon(IconId(5));
        let node = button.layout_node(&mut text, theme);
        let bounds = LogicalRect::from_xywh(
            10.0,
            20.0,
            node.measure(crate::Constraints::UNBOUNDED).width.0,
            36.0,
        );
        let hit = button.hit_region(bounds, PaintLayer::Content, 0);
        let mut input = InputState::default();
        input.focus.set_tab_order([(button.id, true, true)]);
        input.pointer = LogicalPoint::new(15.0, 25.0);
        let down = input.route(InputEvent::PointerDown(MouseButton::Primary), &[hit]);
        assert_eq!(
            button.handle(down.target, &down.event, &mut input, bounds),
            None
        );
        let up = input.route(InputEvent::PointerUp(MouseButton::Primary), &[hit]);
        assert_eq!(
            button.handle(up.target, &up.event, &mut input, bounds),
            Some(WidgetEvent::Activated(button.id))
        );

        let mut paint = PaintBuilder::new();
        button.paint(bounds, button.state(&input), &mut text, theme, &mut paint);
        let commands = paint.finish().into_commands();
        assert!(commands
            .iter()
            .any(|command| matches!(command.op, PaintOp::Icon { id: IconId(5), .. })));
        assert!(commands
            .iter()
            .any(|command| matches!(command.op, PaintOp::Text { .. })));
    }

    #[test]
    fn label_ellipsis_uses_measured_shaped_width() {
        let mut text = TextSystem::with_cache_limits(64, 1024 * 1024);
        let label =
            Label::new(WidgetId(2), "Русское имя файла.txt").with_max_width(LogicalPx(52.0));
        let layout = label.measure(&mut text, Theme::EVERTY_LIGHT);
        assert!(layout.metrics.size.width.0 <= 52.0);
        assert!(layout.metrics.size.width.0 > 0.0);
    }

    #[test]
    fn ellipsis_never_splits_a_grapheme_cluster_at_any_width() {
        let mut text = TextSystem::with_cache_limits(64, 1024 * 1024);
        let style = text_style(Theme::EVERTY_LIGHT);
        let samples = [
            // base + combining acute, repeated
            "Ae\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}",
            // Cyrillic with a combining breve (decomposed й) in a long file name
            "Отчёт_по_проекту_и\u{306}юль_финальная_версия_копия_2.docx",
            // ZWJ family emoji and a regional-indicator flag
            "a👨\u{200d}👩\u{200d}👧\u{200d}👦b🇷🇺c👨\u{200d}👩\u{200d}👧\u{200d}👦d",
        ];
        for sample in samples {
            let boundaries: Vec<usize> = sample
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .chain([sample.len()])
                .collect();
            let full = text.shape(sample, &style, None).metrics.size.width.0;
            let mut width = 0.0;
            while width <= full + 1.0 {
                let cut = ellipsized_text(sample, LogicalPx(width), &style, &mut text);
                if let Some(prefix) = cut.strip_suffix('…') {
                    assert!(sample.starts_with(prefix), "{cut:?} is not a prefix");
                    assert!(
                        boundaries.contains(&prefix.len()),
                        "{sample:?} split inside a cluster at width {width}: {cut:?}"
                    );
                    let measured = text.shape(&cut, &style, None).metrics.size.width.0;
                    assert!(measured <= width + 1e-6, "{cut:?} too wide at {width}");
                } else {
                    assert!(cut.is_empty(), "unexpected result {cut:?}");
                }
                width += 1.0;
            }
        }
    }

    #[test]
    fn disabled_button_never_activates_and_has_disabled_hit_region() {
        let mut button = Button::new(WidgetId(4), "Disabled").enabled(false);
        let rect = LogicalRect::from_xywh(0.0, 0.0, 80.0, 36.0);
        let mut input = InputState {
            pointer: LogicalPoint::new(5.0, 5.0),
            ..InputState::default()
        };
        let down = input.route(
            InputEvent::PointerDown(MouseButton::Primary),
            &[button.hit_region(rect, PaintLayer::Content, 0)],
        );
        assert_eq!(
            button.handle(down.target, &down.event, &mut input, rect),
            None
        );
        assert_eq!(down.target, None);
    }

    #[test]
    fn window_focus_loss_cancels_pointer_and_keyboard_button_arming() {
        let id = WidgetId(12);
        let mut button = Button::new(id, "Action");
        let rect = LogicalRect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let hit = button.hit_region(rect, PaintLayer::Content, 0);
        let mut input = InputState {
            pointer: LogicalPoint::new(10.0, 10.0),
            ..InputState::default()
        };
        input.focus.set_tab_order([(id, true, true)]);

        let down = input.route(InputEvent::PointerDown(MouseButton::Primary), &[hit]);
        button.handle(down.target, &down.event, &mut input, rect);
        assert!(button.state(&input).pressed);
        assert_eq!(input.pointer_capture, Some(id));

        let lost = input.route(InputEvent::WindowFocused(false), &[hit]);
        button.handle(lost.target, &lost.event, &mut input, rect);
        assert!(!button.state(&input).pressed);
        assert_eq!(input.pointer_capture, None);
        let up = input.route(InputEvent::PointerUp(MouseButton::Primary), &[hit]);
        assert_eq!(button.handle(up.target, &up.event, &mut input, rect), None);

        let key_down = InputEvent::Key {
            key: Key::Enter,
            pressed: true,
            modifiers: crate::Modifiers::default(),
        };
        button.handle(Some(id), &key_down, &mut input, rect);
        assert!(button.state(&input).pressed);
        button.handle(None, &InputEvent::WindowFocused(false), &mut input, rect);
        let key_up = InputEvent::Key {
            key: Key::Enter,
            pressed: false,
            modifiers: crate::Modifiers::default(),
        };
        assert_eq!(button.handle(Some(id), &key_up, &mut input, rect), None);
        assert!(!button.state(&input).pressed);
    }

    #[test]
    fn pointer_cancel_disarms_button_before_late_release() {
        let id = WidgetId(13);
        let mut button = Button::new(id, "Action");
        let rect = LogicalRect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let hit = button.hit_region(rect, PaintLayer::Content, 0);
        let mut input = InputState {
            pointer: LogicalPoint::new(10.0, 10.0),
            ..InputState::default()
        };
        let down = input.route(InputEvent::PointerDown(MouseButton::Primary), &[hit]);
        button.handle(down.target, &down.event, &mut input, rect);
        assert!(button.state(&input).pressed);
        let canceled = input.route(InputEvent::PointerCancel, &[hit]);
        button.handle(canceled.target, &canceled.event, &mut input, rect);
        assert!(!button.state(&input).pressed);
        let up = input.route(InputEvent::PointerUp(MouseButton::Primary), &[hit]);
        assert_eq!(button.handle(up.target, &up.event, &mut input, rect), None);
    }

    #[test]
    fn icon_button_uses_the_same_control_contract() {
        let mut text = TextSystem::with_cache_limits(16, 4096);
        let icon = IconButton::new(WidgetId(8), IconId(2));
        let size = icon
            .layout_node(&mut text, Theme::EVERTY_LIGHT)
            .measure(crate::Constraints::UNBOUNDED);
        assert!(size.width.0 >= 28.0);
        assert_eq!(
            size.height.0,
            Theme::EVERTY_LIGHT.metrics.control_height_md.0
        );

        let large = IconButton::new(WidgetId(9), IconId(3))
            .with_icon_size(crate::LogicalPx(24.0))
            .layout_node(&mut text, Theme::EVERTY_LIGHT)
            .measure(crate::Constraints::UNBOUNDED);
        assert_eq!(large.width.0, 48.0);
        assert_eq!(large.height.0, 40.0);
    }

    #[test]
    fn key_up_after_focus_moved_away_cannot_activate_a_stale_armed_button() {
        let id = WidgetId(21);
        let other = WidgetId(22);
        let mut button = Button::new(id, "Action");
        let rect = LogicalRect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let mut input = InputState::default();
        let modifiers = crate::Modifiers::default();
        let key = |key, pressed| InputEvent::Key {
            key,
            pressed,
            modifiers,
        };
        button.handle(Some(id), &key(Key::Enter, true), &mut input, rect);
        assert!(button.state(&input).pressed);
        // Tab moves focus to another widget while Enter is still held.
        button.handle(Some(other), &key(Key::Tab, true), &mut input, rect);
        assert!(!button.state(&input).pressed, "focus loss must disarm");
        button.handle(Some(id), &key(Key::Enter, false), &mut input, rect);
        // Focus returns later; a lone key-up must not click.
        assert_eq!(
            button.handle(Some(id), &key(Key::Enter, false), &mut input, rect),
            None
        );
    }

    fn first_fill(button: &Button, state: ButtonState) -> Color {
        let mut text = TextSystem::with_cache_limits(16, 4096);
        let mut paint = PaintBuilder::new();
        button.paint(
            LogicalRect::from_xywh(0.0, 0.0, 100.0, 36.0),
            state,
            &mut text,
            Theme::EVERTY_LIGHT,
            &mut paint,
        );
        match paint.finish().into_commands()[0].op {
            PaintOp::FillRect { color, .. } => color,
            _ => panic!("button must start with its background fill"),
        }
    }

    #[test]
    fn button_variants_use_status_tokens_for_every_interaction_state() {
        let colors = Theme::EVERTY_LIGHT.colors;
        let idle = ButtonState {
            enabled: true,
            ..ButtonState::default()
        };
        let hovered = ButtonState {
            hovered: true,
            ..idle
        };
        let pressed = ButtonState {
            pressed: true,
            ..idle
        };
        let flat = Button::new(WidgetId(9), "Flat").variant(ButtonVariant::Flat);
        assert_eq!(
            first_fill(&flat, idle).0[3],
            0,
            "flat is transparent at rest"
        );
        assert_eq!(first_fill(&flat, hovered), colors.surface_hover);
        assert_eq!(first_fill(&flat, pressed), colors.surface_pressed);
        let default = Button::new(WidgetId(1), "Close");
        assert_eq!(first_fill(&default, idle), colors.surface);
        assert_eq!(first_fill(&default, hovered), colors.surface_hover);
        assert_eq!(first_fill(&default, pressed), colors.surface_pressed);
        for (variant, base) in [
            (ButtonVariant::Primary, colors.accent),
            (ButtonVariant::Danger, colors.danger),
        ] {
            let button = Button::new(WidgetId(2), "Go").variant(variant);
            assert_eq!(first_fill(&button, idle), base);
            let hover = first_fill(&button, hovered);
            let press = first_fill(&button, pressed);
            assert_ne!(hover, base);
            assert_ne!(press, hover, "pressed is darker than hovered");
            let disabled = first_fill(&button, ButtonState::default());
            assert_ne!(disabled, base, "disabled variants are washed out");
        }
    }

    #[test]
    fn icon_tint_applies_only_when_enabled() {
        let mut text = TextSystem::with_cache_limits(16, 4096);
        let tint = Color::rgba(1, 2, 3, 255);
        let mut button = IconButton::new(WidgetId(3), IconId(1));
        button.set_icon_tint(Some(tint));
        for (enabled, expected) in [
            (true, tint),
            (false, Theme::EVERTY_LIGHT.colors.text_disabled),
        ] {
            let mut paint = PaintBuilder::new();
            button.paint(
                LogicalRect::from_xywh(0.0, 0.0, 40.0, 36.0),
                ButtonState {
                    enabled,
                    ..ButtonState::default()
                },
                &mut text,
                Theme::EVERTY_LIGHT,
                &mut paint,
            );
            let icon_tint = paint
                .finish()
                .into_commands()
                .into_iter()
                .find_map(|command| match command.op {
                    PaintOp::Icon { tint, .. } => Some(tint),
                    _ => None,
                })
                .unwrap();
            assert_eq!(icon_tint, expected);
        }
    }

    #[test]
    fn checkbox_toggle_and_indeterminate_cycle() {
        let mut checkbox = Checkbox::new(WidgetId(10), "Remember me");
        assert_eq!(checkbox.state, CheckState::Unchecked);

        let mut input = InputState::default();
        let rect = LogicalRect::from_xywh(0.0, 0.0, 150.0, 30.0);

        // Click to toggle
        let down = InputEvent::PointerDown(MouseButton::Primary);
        checkbox.handle(Some(WidgetId(10)), &down, &mut input, rect);
        input.pointer = LogicalPoint::new(10.0, 10.0);
        let up = InputEvent::PointerUp(MouseButton::Primary);
        assert_eq!(
            checkbox.handle(Some(WidgetId(10)), &up, &mut input, rect),
            Some(CheckState::Checked)
        );
        assert_eq!(checkbox.state, CheckState::Checked);

        // Space key toggles
        let space_down = InputEvent::Key {
            key: Key::Space,
            pressed: true,
            modifiers: Modifiers::default(),
        };
        checkbox.handle(Some(WidgetId(10)), &space_down, &mut input, rect);
        let space_up = InputEvent::Key {
            key: Key::Space,
            pressed: false,
            modifiers: Modifiers::default(),
        };
        assert_eq!(
            checkbox.handle(Some(WidgetId(10)), &space_up, &mut input, rect),
            Some(CheckState::Unchecked)
        );

        // Indeterminate state toggles to Unchecked
        checkbox.state = CheckState::Indeterminate;
        assert_eq!(checkbox.state.toggle(), CheckState::Unchecked);
    }

    #[test]
    fn switch_toggle_pointer_and_keyboard() {
        let mut switch = Switch::new(WidgetId(20)).with_label("Notifications");
        assert!(!switch.on);

        let mut input = InputState::default();
        let rect = LogicalRect::from_xywh(0.0, 0.0, 150.0, 30.0);

        // Pointer click
        let down = InputEvent::PointerDown(MouseButton::Primary);
        switch.handle(Some(WidgetId(20)), &down, &mut input, rect);
        input.pointer = LogicalPoint::new(10.0, 10.0);
        let up = InputEvent::PointerUp(MouseButton::Primary);
        assert_eq!(
            switch.handle(Some(WidgetId(20)), &up, &mut input, rect),
            Some(true)
        );
        assert!(switch.on);

        // Enter key toggles
        let enter_down = InputEvent::Key {
            key: Key::Enter,
            pressed: true,
            modifiers: Modifiers::default(),
        };
        switch.handle(Some(WidgetId(20)), &enter_down, &mut input, rect);
        let enter_up = InputEvent::Key {
            key: Key::Enter,
            pressed: false,
            modifiers: Modifiers::default(),
        };
        assert_eq!(
            switch.handle(Some(WidgetId(20)), &enter_up, &mut input, rect),
            Some(false)
        );
        assert!(!switch.on);
    }

    #[test]
    fn tab_bar_selection_closing_and_keyboard_navigation() {
        let tabs = vec![
            TabItem::new(WidgetId(1), "Tab 1").closable(WidgetId(101)),
            TabItem::new(WidgetId(2), "Tab 2").closable(WidgetId(102)),
            TabItem::new(WidgetId(3), "Tab 3").closable(WidgetId(103)),
        ];
        let mut bar = TabBar::new(WidgetId(10), tabs, Some(WidgetId(1)));

        // Arrow navigation
        assert_eq!(
            bar.handle_key(Key::ArrowRight, Modifiers::default()),
            Some(TabEvent::Selected(WidgetId(2)))
        );
        assert_eq!(bar.active_tab, Some(WidgetId(2)));

        assert_eq!(
            bar.handle_key(Key::ArrowLeft, Modifiers::default()),
            Some(TabEvent::Selected(WidgetId(1)))
        );
        assert_eq!(bar.active_tab, Some(WidgetId(1)));

        // Pointer down on close button closes tab, does not select
        assert_eq!(
            bar.handle_pointer_down(WidgetId(102)),
            Some(TabEvent::Closed(WidgetId(2)))
        );
        assert_eq!(bar.tabs.len(), 2);
        assert_eq!(bar.tabs[0].id, WidgetId(1));
        assert_eq!(bar.tabs[1].id, WidgetId(3));

        // Pointer down on Tab 3 selects it
        assert_eq!(
            bar.handle_pointer_down(WidgetId(3)),
            Some(TabEvent::Selected(WidgetId(3)))
        );
        assert_eq!(bar.active_tab, Some(WidgetId(3)));
    }
}
