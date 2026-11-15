//! Reusable scroll state and viewport/content geometry.

use crate::{
    CursorIcon, HitRegion, LogicalPoint, LogicalPx, LogicalRect, LogicalSize, PaintLayer, WidgetId,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    pub offset: LogicalPoint,
    pub viewport: LogicalSize,
    pub content: LogicalSize,
}

impl ScrollState {
    pub fn set_metrics(&mut self, viewport: LogicalSize, content: LogicalSize) {
        self.viewport = non_negative_size(viewport);
        self.content = non_negative_size(content);
        self.clamp();
    }

    pub fn set_offset(&mut self, offset: LogicalPoint) {
        self.offset = offset;
        self.clamp();
    }

    /// Scroll by a wheel/trackpad delta. NaN components are ignored and
    /// infinite ones saturate at the corresponding end. Returns whether the
    /// offset changed, so callers can skip a redraw when already at a limit.
    pub fn scroll_by(&mut self, delta: LogicalPoint) -> bool {
        let before = self.offset;
        if !delta.x.0.is_nan() {
            self.offset.x.0 += delta.x.0;
        }
        if !delta.y.0.is_nan() {
            self.offset.y.0 += delta.y.0;
        }
        self.clamp();
        self.offset != before
    }

    pub fn ensure_visible(&mut self, item: LogicalRect) {
        if [item.left(), item.top(), item.right(), item.bottom()]
            .iter()
            .any(|value| value.is_nan())
        {
            return;
        }
        if item.left() < self.offset.x.0 {
            self.offset.x.0 = item.left();
        } else if item.right() > self.offset.x.0 + self.viewport.width.0 {
            self.offset.x.0 = item.right() - self.viewport.width.0;
        }
        if item.top() < self.offset.y.0 {
            self.offset.y.0 = item.top();
        } else if item.bottom() > self.offset.y.0 + self.viewport.height.0 {
            self.offset.y.0 = item.bottom() - self.viewport.height.0;
        }
        self.clamp();
    }

    pub fn max_offset(&self) -> LogicalPoint {
        LogicalPoint::new(
            (self.content.width.0 - self.viewport.width.0).max(0.0),
            (self.content.height.0 - self.viewport.height.0).max(0.0),
        )
    }

    pub fn clamp(&mut self) {
        let max = self.max_offset();
        self.offset = LogicalPoint::new(
            finite(self.offset.x.0).clamp(0.0, max.x.0),
            finite(self.offset.y.0).clamp(0.0, max.y.0),
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAreaLayout {
    pub viewport: LogicalRect,
    pub content: LogicalRect,
    pub clip: LogicalRect,
    pub vertical_scrollbar: Option<LogicalRect>,
    pub horizontal_scrollbar: Option<LogicalRect>,
    pub vertical_track: Option<LogicalRect>,
    pub horizontal_track: Option<LogicalRect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollArea {
    pub state: ScrollState,
    pub scrollbar_extent: LogicalPx,
    pub minimum_thumb: LogicalPx,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self {
            state: ScrollState::default(),
            scrollbar_extent: LogicalPx(10.0),
            minimum_thumb: LogicalPx(18.0),
        }
    }
}

impl ScrollArea {
    pub fn layout(&mut self, bounds: LogicalRect, content_size: LogicalSize) -> ScrollAreaLayout {
        let bar = self.scrollbar_extent.0.max(0.0);
        let mut vertical = content_size.height.0 > bounds.size.height.0;
        let mut horizontal = content_size.width.0 > bounds.size.width.0;
        let mut viewport_size = bounds.size;
        if vertical {
            viewport_size.width.0 = (viewport_size.width.0 - bar).max(0.0);
        }
        if horizontal {
            viewport_size.height.0 = (viewport_size.height.0 - bar).max(0.0);
        }
        // A reserved scrollbar can induce overflow on the perpendicular axis.
        vertical |= content_size.height.0 > viewport_size.height.0;
        horizontal |= content_size.width.0 > viewport_size.width.0;
        viewport_size.width.0 = (bounds.size.width.0 - if vertical { bar } else { 0.0 }).max(0.0);
        viewport_size.height.0 =
            (bounds.size.height.0 - if horizontal { bar } else { 0.0 }).max(0.0);
        self.state.set_metrics(viewport_size, content_size);
        let viewport = LogicalRect::new(bounds.origin, viewport_size);
        let content = LogicalRect::from_xywh(
            viewport.left() - self.state.offset.x.0,
            viewport.top() - self.state.offset.y.0,
            content_size.width.0,
            content_size.height.0,
        );
        let vertical_track = vertical.then(|| {
            LogicalRect::from_xywh(viewport.right(), bounds.top(), bar, viewport.size.height.0)
        });
        let vertical_scrollbar = vertical_track.map(|track| {
            scrollbar_thumb(
                track,
                ScrollAxis::Vertical,
                LogicalPx(viewport.size.height.0),
                LogicalPx(content_size.height.0),
                LogicalPx(self.state.offset.y.0),
                self.minimum_thumb,
            )
        });
        let horizontal_track = horizontal.then(|| {
            LogicalRect::from_xywh(
                bounds.left(),
                viewport.bottom(),
                viewport.size.width.0,
                bar,
            )
        });
        let horizontal_scrollbar = horizontal_track.map(|track| {
            scrollbar_thumb(
                track,
                ScrollAxis::Horizontal,
                LogicalPx(viewport.size.width.0),
                LogicalPx(content_size.width.0),
                LogicalPx(self.state.offset.x.0),
                self.minimum_thumb,
            )
        });
        ScrollAreaLayout {
            viewport,
            content,
            clip: viewport,
            vertical_scrollbar,
            horizontal_scrollbar,
            vertical_track,
            horizontal_track,
        }
    }

    pub fn scroll_by(&mut self, delta: LogicalPoint) -> bool {
        self.state.scroll_by(delta)
    }

    pub fn handle_pointer_down(
        &mut self,
        controller: &mut ScrollbarController,
        point: LogicalPoint,
        layout: &ScrollAreaLayout,
    ) -> ScrollAction {
        controller.handle_pointer_down(point, layout, &mut self.state)
    }

    pub fn handle_pointer_moved(
        &mut self,
        controller: &mut ScrollbarController,
        point: LogicalPoint,
        layout: &ScrollAreaLayout,
    ) -> bool {
        controller.handle_pointer_moved(point, layout, &mut self.state)
    }

    pub fn handle_pointer_up(&mut self, controller: &mut ScrollbarController) -> bool {
        controller.handle_pointer_up()
    }
}

impl ScrollAreaLayout {
    /// Generates hit regions for scrollbar tracks and thumbs.
    pub fn hit_regions(
        &self,
        vertical_track_id: Option<WidgetId>,
        vertical_thumb_id: Option<WidgetId>,
        horizontal_track_id: Option<WidgetId>,
        horizontal_thumb_id: Option<WidgetId>,
        layer: PaintLayer,
    ) -> Vec<HitRegion> {
        let mut regions = Vec::with_capacity(4);
        if let (Some(track), Some(id)) = (self.vertical_track, vertical_track_id) {
            regions.push(HitRegion {
                id,
                rect: track,
                layer,
                paint_order: 10,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            });
        }
        if let (Some(thumb), Some(id)) = (self.vertical_scrollbar, vertical_thumb_id) {
            regions.push(HitRegion {
                id,
                rect: thumb,
                layer,
                paint_order: 11,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            });
        }
        if let (Some(track), Some(id)) = (self.horizontal_track, horizontal_track_id) {
            regions.push(HitRegion {
                id,
                rect: track,
                layer,
                paint_order: 10,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            });
        }
        if let (Some(thumb), Some(id)) = (self.horizontal_scrollbar, horizontal_thumb_id) {
            regions.push(HitRegion {
                id,
                rect: thumb,
                layer,
                paint_order: 11,
                enabled: true,
                focusable: false,
                cursor: CursorIcon::Default,
            });
        }
        regions
    }
}

/// Action resulting from pointer interaction with a scrollbar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollAction {
    None,
    ThumbDragStarted { axis: ScrollAxis },
    PageScrolled { axis: ScrollAxis, forward: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DragState {
    axis: ScrollAxis,
    grab_offset: f64,
}

/// Manages interactive thumb dragging and track clicking for scrollbars.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarController {
    drag: Option<DragState>,
}

impl ScrollbarController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn dragging_axis(&self) -> Option<ScrollAxis> {
        self.drag.map(|d| d.axis)
    }

    /// Handles pointer-down on a scrollbar track or thumb.
    pub fn handle_pointer_down(
        &mut self,
        point: LogicalPoint,
        layout: &ScrollAreaLayout,
        state: &mut ScrollState,
    ) -> ScrollAction {
        if [point.x.0, point.y.0].iter().any(|v| v.is_nan()) {
            return ScrollAction::None;
        }

        // Check vertical scrollbar
        if let (Some(track), Some(thumb)) = (layout.vertical_track, layout.vertical_scrollbar) {
            if thumb.contains(point) {
                let grab = point.y.0 - thumb.top();
                self.drag = Some(DragState {
                    axis: ScrollAxis::Vertical,
                    grab_offset: grab,
                });
                return ScrollAction::ThumbDragStarted {
                    axis: ScrollAxis::Vertical,
                };
            } else if track.contains(point) {
                let forward = point.y.0 > thumb.bottom();
                let delta = if forward {
                    layout.viewport.size.height.0
                } else {
                    -layout.viewport.size.height.0
                };
                let changed = state.scroll_by(LogicalPoint::new(0.0, delta));
                return if changed {
                    ScrollAction::PageScrolled {
                        axis: ScrollAxis::Vertical,
                        forward,
                    }
                } else {
                    ScrollAction::None
                };
            }
        }

        // Check horizontal scrollbar
        if let (Some(track), Some(thumb)) = (layout.horizontal_track, layout.horizontal_scrollbar) {
            if thumb.contains(point) {
                let grab = point.x.0 - thumb.left();
                self.drag = Some(DragState {
                    axis: ScrollAxis::Horizontal,
                    grab_offset: grab,
                });
                return ScrollAction::ThumbDragStarted {
                    axis: ScrollAxis::Horizontal,
                };
            } else if track.contains(point) {
                let forward = point.x.0 > thumb.right();
                let delta = if forward {
                    layout.viewport.size.width.0
                } else {
                    -layout.viewport.size.width.0
                };
                let changed = state.scroll_by(LogicalPoint::new(delta, 0.0));
                return if changed {
                    ScrollAction::PageScrolled {
                        axis: ScrollAxis::Horizontal,
                        forward,
                    }
                } else {
                    ScrollAction::None
                };
            }
        }

        ScrollAction::None
    }

    /// Handles pointer-moved while dragging a thumb. Returns true if offset changed.
    pub fn handle_pointer_moved(
        &mut self,
        point: LogicalPoint,
        layout: &ScrollAreaLayout,
        state: &mut ScrollState,
    ) -> bool {
        let Some(drag) = self.drag else {
            return false;
        };
        if [point.x.0, point.y.0].iter().any(|v| v.is_nan()) {
            return false;
        }

        match drag.axis {
            ScrollAxis::Vertical => {
                let (Some(track), Some(thumb)) = (layout.vertical_track, layout.vertical_scrollbar)
                else {
                    return false;
                };
                let track_extent = track.size.height.0;
                let thumb_extent = thumb.size.height.0;
                let scrollable_track = (track_extent - thumb_extent).max(0.0);
                let scrollable_content = (state.content.height.0 - state.viewport.height.0).max(0.0);

                if scrollable_track <= 0.0 || scrollable_content <= 0.0 {
                    return false;
                }

                let desired_thumb_top = point.y.0 - drag.grab_offset;
                let progress = ((desired_thumb_top - track.top()) / scrollable_track).clamp(0.0, 1.0);
                let new_offset_y = progress * scrollable_content;

                let old_offset_y = state.offset.y.0;
                state.offset.y.0 = new_offset_y;
                state.clamp();
                (state.offset.y.0 - old_offset_y).abs() > f64::EPSILON
            }
            ScrollAxis::Horizontal => {
                let (Some(track), Some(thumb)) =
                    (layout.horizontal_track, layout.horizontal_scrollbar)
                else {
                    return false;
                };
                let track_extent = track.size.width.0;
                let thumb_extent = thumb.size.width.0;
                let scrollable_track = (track_extent - thumb_extent).max(0.0);
                let scrollable_content = (state.content.width.0 - state.viewport.width.0).max(0.0);

                if scrollable_track <= 0.0 || scrollable_content <= 0.0 {
                    return false;
                }

                let desired_thumb_left = point.x.0 - drag.grab_offset;
                let progress =
                    ((desired_thumb_left - track.left()) / scrollable_track).clamp(0.0, 1.0);
                let new_offset_x = progress * scrollable_content;

                let old_offset_x = state.offset.x.0;
                state.offset.x.0 = new_offset_x;
                state.clamp();
                (state.offset.x.0 - old_offset_x).abs() > f64::EPSILON
            }
        }
    }

    /// Releases drag state. Returns true if was dragging.
    pub fn handle_pointer_up(&mut self) -> bool {
        let was_dragging = self.drag.is_some();
        self.drag = None;
        was_dragging
    }

    /// Cancels dragging without changing offset.
    pub fn cancel(&mut self) {
        self.drag = None;
    }
}

/// Thumb bounds in the same logical coordinate space as its track.
pub fn scrollbar_thumb(
    track: LogicalRect,
    axis: ScrollAxis,
    viewport_extent: LogicalPx,
    content_extent: LogicalPx,
    offset: LogicalPx,
    minimum_thumb: LogicalPx,
) -> LogicalRect {
    let (track_start, track_extent) = match axis {
        ScrollAxis::Horizontal => (track.left(), track.size.width.0),
        ScrollAxis::Vertical => (track.top(), track.size.height.0),
    };
    if track_extent <= 0.0 || content_extent.0 <= 0.0 {
        return match axis {
            ScrollAxis::Horizontal => {
                LogicalRect::from_xywh(track_start, track.top(), 0.0, track.size.height.0)
            }
            ScrollAxis::Vertical => {
                LogicalRect::from_xywh(track.left(), track_start, track.size.width.0, 0.0)
            }
        };
    }
    let thumb_extent = (track_extent * (viewport_extent.0 / content_extent.0))
        .max(minimum_thumb.0)
        .min(track_extent);
    let scrollable_content = (content_extent.0 - viewport_extent.0).max(0.0);
    let scrollable_track = (track_extent - thumb_extent).max(0.0);
    let thumb_start = track_start
        + if scrollable_content > 0.0 {
            offset.0.clamp(0.0, scrollable_content) / scrollable_content * scrollable_track
        } else {
            0.0
        };
    match axis {
        ScrollAxis::Horizontal => {
            LogicalRect::from_xywh(thumb_start, track.top(), thumb_extent, track.size.height.0)
        }
        ScrollAxis::Vertical => {
            LogicalRect::from_xywh(track.left(), thumb_start, track.size.width.0, thumb_extent)
        }
    }
}

fn non_negative_size(size: LogicalSize) -> LogicalSize {
    LogicalSize::new(non_negative(size.width.0), non_negative(size.height.0))
}

fn non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// NaN becomes 0; infinities are kept so `clamp` saturates them at an end
/// instead of snapping a "scroll to the end" request back to the start.
fn finite(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_offsets_clamp_and_ensure_visible() {
        let mut scroll = ScrollState::default();
        scroll.set_metrics(
            LogicalSize::new(100.0, 60.0),
            LogicalSize::new(500.0, 400.0),
        );
        scroll.set_offset(LogicalPoint::new(900.0, -20.0));
        assert_eq!(scroll.offset, LogicalPoint::new(400.0, 0.0));
        scroll.ensure_visible(LogicalRect::from_xywh(20.0, 100.0, 30.0, 20.0));
        assert_eq!(scroll.offset, LogicalPoint::new(20.0, 60.0));
        scroll.scroll_by(LogicalPoint::new(0.0, 1000.0));
        assert_eq!(scroll.offset.y.0, 340.0);
    }

    #[test]
    fn scrollbar_thumb_tracks_viewport_and_offset() {
        let track = LogicalRect::from_xywh(0.0, 0.0, 10.0, 100.0);
        let at_top = scrollbar_thumb(
            track,
            ScrollAxis::Vertical,
            LogicalPx(50.0),
            LogicalPx(200.0),
            LogicalPx(0.0),
            LogicalPx(8.0),
        );
        let at_bottom = scrollbar_thumb(
            track,
            ScrollAxis::Vertical,
            LogicalPx(50.0),
            LogicalPx(200.0),
            LogicalPx(150.0),
            LogicalPx(8.0),
        );
        assert_eq!(at_top, LogicalRect::from_xywh(0.0, 0.0, 10.0, 25.0));
        assert_eq!(at_bottom, LogicalRect::from_xywh(0.0, 75.0, 10.0, 25.0));
    }

    #[test]
    fn scroll_area_returns_shared_viewport_content_and_nested_clip_geometry() {
        let mut area = ScrollArea::default();
        let layout = area.layout(
            LogicalRect::from_xywh(10.0, 20.0, 200.0, 100.0),
            LogicalSize::new(400.0, 300.0),
        );
        assert_eq!(
            layout.viewport,
            LogicalRect::from_xywh(10.0, 20.0, 190.0, 90.0)
        );
        assert_eq!(layout.clip, layout.viewport);
        assert!(layout.vertical_scrollbar.is_some());
        assert!(layout.horizontal_scrollbar.is_some());
        area.scroll_by(LogicalPoint::new(15.0, 25.0));
        let scrolled = area.layout(
            LogicalRect::from_xywh(10.0, 20.0, 200.0, 100.0),
            LogicalSize::new(400.0, 300.0),
        );
        assert_eq!(scrolled.content.origin, LogicalPoint::new(-5.0, -5.0));
    }

    #[test]
    fn wheel_deltas_saturate_clamp_and_ignore_nan() {
        let mut scroll = ScrollState::default();
        scroll.set_metrics(
            LogicalSize::new(100.0, 100.0),
            LogicalSize::new(100.0, 1000.0),
        );
        assert!(scroll.scroll_by(LogicalPoint::new(0.0, 1e300)));
        assert_eq!(scroll.offset.y.0, 900.0, "huge delta clamps to the end");
        assert!(
            !scroll.scroll_by(LogicalPoint::new(0.0, 48.0)),
            "already at the end"
        );
        assert!(scroll.scroll_by(LogicalPoint::new(0.0, f64::NEG_INFINITY)));
        assert_eq!(scroll.offset.y.0, 0.0);
        assert!(scroll.scroll_by(LogicalPoint::new(0.0, f64::INFINITY)));
        assert_eq!(
            scroll.offset.y.0, 900.0,
            "+inf means the end, not the start"
        );
        assert!(!scroll.scroll_by(LogicalPoint::new(f64::NAN, f64::NAN)));
        assert_eq!(scroll.offset.y.0, 900.0, "NaN must not move the view");
        assert!(scroll.scroll_by(LogicalPoint::new(0.0, -200.5)));
        assert_eq!(scroll.offset.y.0, 699.5);
        assert!(
            !scroll.scroll_by(LogicalPoint::new(30.0, 0.0)),
            "no horizontal overflow"
        );
    }

    #[test]
    fn content_smaller_than_viewport_never_scrolls_and_resize_reclamps() {
        let mut area = ScrollArea::default();
        let bounds = LogicalRect::from_xywh(0.0, 0.0, 200.0, 100.0);
        let small = area.layout(bounds, LogicalSize::new(150.0, 60.0));
        assert!(small.vertical_scrollbar.is_none() && small.horizontal_scrollbar.is_none());
        assert_eq!(small.viewport, bounds);
        assert!(!area.scroll_by(LogicalPoint::new(0.0, 500.0)));
        assert_eq!(area.state.offset, LogicalPoint::ZERO);

        let large = LogicalSize::new(150.0, 400.0);
        area.layout(bounds, large);
        area.scroll_by(LogicalPoint::new(0.0, 1e9));
        let at_end = area.state.offset.y.0;
        assert_eq!(at_end, 400.0 - 100.0);
        // Growing the window shrinks the scroll range: the offset follows.
        let taller = area.layout(LogicalRect::from_xywh(0.0, 0.0, 200.0, 250.0), large);
        assert_eq!(area.state.offset.y.0, 150.0);
        assert_eq!(taller.content.bottom(), taller.viewport.bottom());
        // Zero-size and negative bounds stay well-formed.
        let tiny = area.layout(LogicalRect::from_xywh(0.0, 0.0, 0.0, -5.0), large);
        assert!(tiny.viewport.size.width.0 >= 0.0 && tiny.viewport.size.height.0 >= 0.0);
        assert!((0.0..=400.0).contains(&area.state.offset.y.0));
    }

    #[test]
    fn nested_scroll_areas_paint_only_inside_both_viewports() {
        use crate::{
            Color, DpiScale, PaintBuilder, PhysicalSize, Renderer, Rgba8Renderer, TextSystem,
        };
        let mut outer = ScrollArea::default();
        let outer_layout = outer.layout(
            LogicalRect::from_xywh(10.0, 10.0, 60.0, 40.0),
            LogicalSize::new(200.0, 200.0),
        );
        // The inner area extends beyond the outer viewport on the right/bottom.
        let mut inner = ScrollArea::default();
        let inner_layout = inner.layout(
            LogicalRect::from_xywh(
                outer_layout.content.left() + 30.0,
                outer_layout.content.top() + 20.0,
                120.0,
                100.0,
            ),
            LogicalSize::new(300.0, 300.0),
        );
        let mut paint = PaintBuilder::new();
        paint.push_clip(outer_layout.clip);
        paint.push_clip(inner_layout.clip);
        paint.fill_rect(inner_layout.content, Color::rgba(255, 0, 0, 255), 0.0);
        paint.pop_clip().unwrap();
        paint.pop_clip().unwrap();
        let paint = paint.finish();
        assert_eq!(
            paint.commands()[0].clip,
            Some(outer_layout.clip.clip(inner_layout.clip))
        );
        let mut renderer = Rgba8Renderer::new();
        renderer
            .render(
                &paint,
                &mut TextSystem::with_cache_limits(1, 1024),
                DpiScale::ONE,
                PhysicalSize::new(100.0, 80.0),
            )
            .unwrap();
        let visible = outer_layout.clip.clip(inner_layout.clip);
        for y in 0..80 {
            for x in 0..100 {
                let painted = renderer.pixels()[(y * 100 + x) * 4 + 3] != 0;
                let expected = visible.contains(LogicalPoint::new(x as f64 + 0.5, y as f64 + 0.5));
                assert_eq!(painted, expected, "pixel ({x},{y})");
            }
        }
    }

    #[test]
    fn scrollbar_controller_drag_from_start_to_end_sweeps_entire_range() {
        let mut area = ScrollArea::default();
        let bounds = LogicalRect::from_xywh(0.0, 0.0, 200.0, 100.0);
        let content_size = LogicalSize::new(200.0, 500.0);
        let layout = area.layout(bounds, content_size);

        let mut controller = ScrollbarController::new();
        let thumb = layout.vertical_scrollbar.expect("vertical scrollbar should exist");
        let track = layout.vertical_track.expect("vertical track should exist");

        // Click on the thumb
        let action = controller.handle_pointer_down(
            LogicalPoint::new(thumb.left() + 2.0, thumb.top() + 5.0),
            &layout,
            &mut area.state,
        );
        assert_eq!(
            action,
            ScrollAction::ThumbDragStarted {
                axis: ScrollAxis::Vertical
            }
        );
        assert!(controller.is_dragging());

        // Drag to top of track
        controller.handle_pointer_moved(
            LogicalPoint::new(thumb.left() + 2.0, track.top() + 5.0),
            &layout,
            &mut area.state,
        );
        assert_eq!(area.state.offset.y.0, 0.0);

        // Drag to bottom of track
        controller.handle_pointer_moved(
            LogicalPoint::new(thumb.left() + 2.0, track.bottom()),
            &layout,
            &mut area.state,
        );
        let max_y = (500.0 - layout.viewport.size.height.0).max(0.0);
        assert_eq!(area.state.offset.y.0, max_y);

        assert!(controller.handle_pointer_up());
        assert!(!controller.is_dragging());
    }

    #[test]
    fn scrollbar_controller_track_click_pages_forward_and_backward() {
        let mut area = ScrollArea::default();
        let bounds = LogicalRect::from_xywh(0.0, 0.0, 200.0, 100.0);
        let content_size = LogicalSize::new(200.0, 500.0);
        let layout = area.layout(bounds, content_size);

        let mut controller = ScrollbarController::new();
        let thumb = layout.vertical_scrollbar.unwrap();
        let track = layout.vertical_track.unwrap();

        // Click track below thumb -> page forward
        let click_point = LogicalPoint::new(track.left() + 2.0, thumb.bottom() + 10.0);
        let action = controller.handle_pointer_down(click_point, &layout, &mut area.state);
        assert_eq!(
            action,
            ScrollAction::PageScrolled {
                axis: ScrollAxis::Vertical,
                forward: true,
            }
        );
        assert_eq!(area.state.offset.y.0, layout.viewport.size.height.0);

        // Update layout after scroll
        let layout2 = area.layout(bounds, content_size);
        let thumb2 = layout2.vertical_scrollbar.unwrap();

        // Click track above thumb -> page backward
        let click_point2 = LogicalPoint::new(track.left() + 2.0, thumb2.top() - 5.0);
        let action2 = controller.handle_pointer_down(click_point2, &layout2, &mut area.state);
        assert_eq!(
            action2,
            ScrollAction::PageScrolled {
                axis: ScrollAxis::Vertical,
                forward: false,
            }
        );
        assert_eq!(area.state.offset.y.0, 0.0);
    }

    #[test]
    fn scrollbar_controller_zero_content_and_degenerate_track_never_panics() {
        let mut area = ScrollArea::default();
        let bounds = LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let content_size = LogicalSize::new(50.0, 50.0); // no overflow
        let layout = area.layout(bounds, content_size);

        let mut controller = ScrollbarController::new();
        let action = controller.handle_pointer_down(
            LogicalPoint::new(50.0, 50.0),
            &layout,
            &mut area.state,
        );
        assert_eq!(action, ScrollAction::None);
        assert!(!controller.handle_pointer_moved(
            LogicalPoint::new(50.0, 50.0),
            &layout,
            &mut area.state,
        ));
        assert!(!controller.handle_pointer_up());

        // NaN coordinates
        let nan_point = LogicalPoint::new(f64::NAN, f64::NAN);
        assert_eq!(
            controller.handle_pointer_down(nan_point, &layout, &mut area.state),
            ScrollAction::None
        );
        assert!(!controller.handle_pointer_moved(nan_point, &layout, &mut area.state));
    }

    #[test]
    fn scroll_area_hit_regions_emit_track_and_thumb() {
        let mut area = ScrollArea::default();
        let layout = area.layout(
            LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0),
            LogicalSize::new(200.0, 300.0),
        );
        let regions = layout.hit_regions(
            Some(WidgetId(1)),
            Some(WidgetId(2)),
            Some(WidgetId(3)),
            Some(WidgetId(4)),
            PaintLayer::Floating,
        );
        assert_eq!(regions.len(), 4);
        assert_eq!(regions[0].id, WidgetId(1)); // v track
        assert_eq!(regions[1].id, WidgetId(2)); // v thumb
        assert_eq!(regions[2].id, WidgetId(3)); // h track
        assert_eq!(regions[3].id, WidgetId(4)); // h thumb
    }
}
