//! Fixed-row virtualization; memory/work is proportional to visible rows.
//!
//! v1 constraint: every row has the same logical height (`item_extent`). Row
//! geometry, the visible range and point-to-row mapping are therefore closed
//! forms that never touch the model; variable-height rows would need a prefix
//! index and are deliberately not provided.

use std::ops::Range;

use crate::{LogicalPx, LogicalRect, LogicalSize, ScrollState};

/// Stable identifier for a list item to preserve selection across sorting, filtering, and reordering.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId(pub u64);

/// Tracks selection by stable identity rather than volatile list index.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyedSelection<Id = ItemId> {
    pub selected: Option<Id>,
}

impl<Id: Copy + PartialEq> KeyedSelection<Id> {
    pub fn new() -> Self {
        Self { selected: None }
    }

    pub fn with_selected(selected: Option<Id>) -> Self {
        Self { selected }
    }

    pub fn select(&mut self, id: Id) {
        self.selected = Some(id);
    }

    pub fn clear(&mut self) {
        self.selected = None;
    }

    pub fn selected(&self) -> Option<Id> {
        self.selected
    }

    pub fn is_selected(&self, id: &Id) -> bool {
        self.selected.as_ref() == Some(id)
    }

    /// Finds the index of the selected item in the current filtered/sorted items slice.
    pub fn selected_index(&self, items: &[Id]) -> Option<usize> {
        let selected = self.selected?;
        items.iter().position(|item| *item == selected)
    }

    /// Selects an item by its current index in `items`.
    pub fn select_by_index(&mut self, index: usize, items: &[Id]) -> bool {
        if let Some(&item) = items.get(index) {
            self.selected = Some(item);
            true
        } else {
            false
        }
    }

    /// Navigates selection by delta (-1 = up/previous, +1 = down/next) within `items`.
    pub fn navigate(&mut self, delta: isize, items: &[Id]) -> Option<Id> {
        if items.is_empty() {
            return None;
        }
        let current_index = self.selected_index(items);
        let next_index = match (current_index, delta) {
            (Some(idx), _) => {
                let target = idx as isize + delta;
                target.clamp(0, (items.len() - 1) as isize) as usize
            }
            (None, d) if d >= 0 => 0,
            (None, _) => items.len() - 1,
        };
        let new_selected = items[next_index];
        self.selected = Some(new_selected);
        Some(new_selected)
    }

    /// Ensures the currently selected item is scrolled into visible view.
    pub fn ensure_selected_visible(
        &self,
        items: &[Id],
        list: &VirtualList,
        scroll: &mut ScrollState,
    ) -> bool {
        if let Some(index) = self.selected_index(items) {
            list.ensure_visible(index, scroll)
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualList {
    pub item_count: usize,
    pub item_extent: LogicalPx,
    pub overscan: usize,
}

impl VirtualList {
    pub fn new(item_count: usize, item_extent: LogicalPx) -> Self {
        Self {
            item_count,
            item_extent: LogicalPx(if item_extent.0.is_finite() {
                item_extent.0.max(0.0)
            } else {
                0.0
            }),
            overscan: 2,
        }
    }

    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn content_size(&self, viewport_width: LogicalPx) -> LogicalSize {
        LogicalSize::new(
            viewport_width.0.max(0.0),
            self.item_count as f64 * self.item_extent.0,
        )
    }

    /// Returns a clamped half-open item range without iterating the model.
    pub fn visible_range(&self, scroll: &ScrollState) -> Range<usize> {
        if self.item_count == 0 || self.item_extent.0 <= 0.0 || scroll.viewport.height.0 <= 0.0 {
            return 0..0;
        }
        let first_visible = (scroll.offset.y.0.max(0.0) / self.item_extent.0).floor() as usize;
        let end_visible = ((scroll.offset.y.0.max(0.0) + scroll.viewport.height.0)
            / self.item_extent.0)
            .ceil() as usize;
        first_visible
            .saturating_sub(self.overscan)
            .min(self.item_count)
            ..end_visible
                .saturating_add(self.overscan)
                .min(self.item_count)
    }

    /// Row under `point` (logical coordinates, same space as `viewport`), or
    /// `None` outside the viewport, past the last row, or for an empty list.
    /// Selection stays attached to model indices, not to screen positions, so
    /// scrolling, resizing and DPI changes cannot re-map a click.
    pub fn index_at(
        &self,
        point: crate::LogicalPoint,
        viewport: LogicalRect,
        scroll: &ScrollState,
    ) -> Option<usize> {
        if self.item_extent.0 <= 0.0 || !viewport.contains(point) {
            return None;
        }
        let row = ((point.y.0 - viewport.top() + scroll.offset.y.0) / self.item_extent.0).floor();
        (row >= 0.0 && row < self.item_count as f64).then_some(row as usize)
    }

    /// Scroll minimally so `index` is fully visible (keyboard navigation).
    /// Returns whether the offset changed; out-of-range indices are ignored.
    pub fn ensure_visible(&self, index: usize, scroll: &mut ScrollState) -> bool {
        if index >= self.item_count || self.item_extent.0 <= 0.0 {
            return false;
        }
        let before = scroll.offset;
        let top = index as f64 * self.item_extent.0;
        scroll.ensure_visible(LogicalRect::from_xywh(
            0.0,
            top,
            scroll.viewport.width.0,
            self.item_extent.0,
        ));
        scroll.offset != before
    }

    pub fn item_rect(
        &self,
        index: usize,
        viewport: LogicalRect,
        scroll: &ScrollState,
    ) -> Option<LogicalRect> {
        if index >= self.item_count || self.item_extent.0 <= 0.0 {
            return None;
        }
        let y = viewport.top() + index as f64 * self.item_extent.0 - scroll.offset.y.0;
        Some(LogicalRect::from_xywh(
            viewport.left(),
            y,
            viewport.size.width.0,
            self.item_extent.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogicalPoint;

    #[test]
    fn virtual_range_is_bounded_for_one_million_model_entries() {
        let list = VirtualList::new(1_000_000, LogicalPx(24.0)).with_overscan(3);
        let mut scroll = ScrollState::default();
        scroll.set_metrics(
            LogicalSize::new(800.0, 480.0),
            list.content_size(LogicalPx(800.0)),
        );
        scroll.set_offset(LogicalPoint::new(0.0, 12_000_000.0));
        let visible = list.visible_range(&scroll);
        assert_eq!(visible.start, 499_997);
        assert_eq!(visible.end, 500_023);
        assert!(visible.len() < 30);
    }

    #[test]
    fn virtual_item_geometry_accounts_for_scroll_offset() {
        let list = VirtualList::new(100, LogicalPx(20.0));
        let mut scroll = ScrollState::default();
        scroll.set_metrics(
            LogicalSize::new(200.0, 100.0),
            list.content_size(LogicalPx(200.0)),
        );
        scroll.set_offset(LogicalPoint::new(0.0, 35.0));
        assert_eq!(
            list.item_rect(2, LogicalRect::from_xywh(10.0, 5.0, 200.0, 100.0), &scroll),
            Some(LogicalRect::from_xywh(10.0, 10.0, 200.0, 20.0))
        );
    }

    fn scrolled(list: &VirtualList, viewport: LogicalSize, offset: f64) -> ScrollState {
        let mut scroll = ScrollState::default();
        scroll.set_metrics(viewport, list.content_size(LogicalPx(viewport.width.0)));
        scroll.set_offset(LogicalPoint::new(0.0, offset));
        scroll
    }

    #[test]
    fn ranges_are_bounded_and_exact_for_empty_single_and_huge_models() {
        let viewport = LogicalSize::new(400.0, 100.0);
        for (count, offset, expected_visible) in [
            (0usize, 0.0, 0..0),
            (1, 0.0, 0..1),
            (1, 1e9, 0..1),
            (100, 0.0, 0..7),
            (100, 1e9, 93..100),
            (10_000, 120_000.0, 4_998..5_007),
            (100_000, 1_200_000.0, 49_998..50_007),
            (1_000_000, 11_999_990.0, 499_997..500_006),
        ] {
            let list = VirtualList::new(count, LogicalPx(24.0)).with_overscan(2);
            let scroll = scrolled(&list, viewport, offset);
            let range = list.visible_range(&scroll);
            assert_eq!(range, expected_visible, "{count} rows at {offset}");
            assert!(
                range.len() <= 9,
                "visible work must not scale with the model"
            );
        }
    }

    #[test]
    fn last_row_ends_exactly_at_the_viewport_edge_for_a_million_rows() {
        let list = VirtualList::new(1_000_000, LogicalPx(24.0));
        let viewport = LogicalRect::from_xywh(5.0, 7.0, 400.0, 100.0);
        let scroll = scrolled(&list, viewport.size, f64::INFINITY);
        let last = list.item_rect(999_999, viewport, &scroll).unwrap();
        assert_eq!(last.bottom(), viewport.bottom());
        assert_eq!(
            list.index_at(
                LogicalPoint::new(10.0, viewport.bottom() - 0.5),
                viewport,
                &scroll
            ),
            Some(999_999)
        );
        assert_eq!(
            list.index_at(
                LogicalPoint::new(10.0, viewport.bottom()),
                viewport,
                &scroll
            ),
            None
        );
    }

    #[test]
    fn index_mapping_round_trips_item_rects_and_survives_resize() {
        let list = VirtualList::new(10_000, LogicalPx(28.0));
        let mut viewport = LogicalRect::from_xywh(0.0, 40.0, 300.0, 280.0);
        let mut scroll = scrolled(&list, viewport.size, 1_234.5);
        for index in list.visible_range(&scroll) {
            let rect = list.item_rect(index, viewport, &scroll).unwrap();
            let centre =
                LogicalPoint::new(rect.left() + 1.0, rect.top() + rect.size.height.0 / 2.0);
            if viewport.contains(centre) {
                assert_eq!(list.index_at(centre, viewport, &scroll), Some(index));
            }
        }
        // The row under a fixed point changes only because of scrolling.
        let probe = LogicalPoint::new(10.0, 100.0);
        let before = list.index_at(probe, viewport, &scroll);
        viewport.size.height.0 += 120.0;
        scroll.set_metrics(
            viewport.size,
            list.content_size(LogicalPx(viewport.size.width.0)),
        );
        assert_eq!(list.index_at(probe, viewport, &scroll), before);
    }

    #[test]
    fn keyboard_navigation_scrolls_minimally_and_ignores_invalid_indices() {
        let list = VirtualList::new(1_000, LogicalPx(20.0));
        let mut scroll = scrolled(&list, LogicalSize::new(200.0, 100.0), 0.0);
        assert!(
            !list.ensure_visible(4, &mut scroll),
            "row 4 is already visible"
        );
        assert!(list.ensure_visible(9, &mut scroll));
        assert_eq!(scroll.offset.y.0, 100.0, "row 9 bottom == viewport bottom");
        assert!(list.ensure_visible(2, &mut scroll));
        assert_eq!(scroll.offset.y.0, 40.0, "row 2 top == viewport top");
        assert!(!list.ensure_visible(1_000, &mut scroll));
        assert!(!VirtualList::new(0, LogicalPx(20.0)).ensure_visible(0, &mut scroll));
    }

    #[test]
    fn rows_tile_without_seams_at_every_required_dpi_scale() {
        use crate::DpiScale;
        let list = VirtualList::new(100_000, LogicalPx(28.0));
        let viewport = LogicalRect::from_xywh(0.0, 0.0, 300.0, 280.0);
        let scroll = scrolled(&list, viewport.size, 54_321.7);
        for factor in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let scale = DpiScale::new(factor).unwrap();
            let range = list.visible_range(&scroll);
            let mut previous_bottom = None;
            for index in range {
                let snapped = scale.snap_rect(list.item_rect(index, viewport, &scroll).unwrap());
                if let Some(bottom) = previous_bottom {
                    // Compare device pixels: the rect's origin+size form is
                    // only ulp-exact in logical units.
                    assert_eq!(
                        (snapped.top() * factor).round(),
                        bottom,
                        "{factor}x seam before row {index}"
                    );
                }
                assert!(snapped.size.height.0 > 0.0);
                previous_bottom = Some((snapped.bottom() * factor).round());
            }
            // Logical range and mapping do not depend on the scale at all.
            assert_eq!(list.visible_range(&scroll), list.visible_range(&scroll));
        }
    }

    #[test]
    fn keyed_selection_preserves_identity_across_reordering_and_filtering() {
        let device_a = ItemId(101);
        let device_b = ItemId(102);
        let device_c = ItemId(103);

        // Initial list: [A, B, C]
        let items_initial = vec![device_a, device_b, device_c];
        let mut selection = KeyedSelection::new();
        selection.select(device_b);
        assert_eq!(selection.selected(), Some(device_b));
        assert_eq!(selection.selected_index(&items_initial), Some(1));

        // User searches or filters: only [C, B] match
        let items_filtered = vec![device_c, device_b];
        // Selection remains attached to device B, but its index is now 1 in the filtered list
        assert_eq!(selection.selected(), Some(device_b));
        assert_eq!(selection.selected_index(&items_filtered), Some(1));

        // User sorts by name descending: [C, B, A]
        let items_sorted = vec![device_c, device_b, device_a];
        assert_eq!(selection.selected(), Some(device_b));
        assert_eq!(selection.selected_index(&items_sorted), Some(1));

        // Navigate down (+1) -> moves to device A
        assert_eq!(selection.navigate(1, &items_sorted), Some(device_a));
        assert_eq!(selection.selected(), Some(device_a));

        // Filter where device A is omitted: [C, B]
        assert_eq!(selection.selected_index(&items_filtered), None);
        // But the ID is still preserved in case the filter changes back!
        assert_eq!(selection.selected(), Some(device_a));
    }
}
