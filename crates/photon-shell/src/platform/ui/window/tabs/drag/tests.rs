use super::super::super::{TabPositionAnimation, tab_position_animation_offset};
use super::super::tab_navigation_offset;
use super::TabDragAxis;
use super::{
    ExternalDropTargetKind, animation_retarget_offset, external_drop_target_kind, insert_id_at,
    is_over_or_moving_toward_bounds, midpoint_insertion_index, plan_tab_transfer,
    stable_slot_midpoint,
};

#[test]
fn tab_navigation_uses_the_layout_axis() {
    assert_eq!(
        tab_navigation_offset("left", TabDragAxis::Horizontal),
        Some(-1)
    );
    assert_eq!(
        tab_navigation_offset("right", TabDragAxis::Horizontal),
        Some(1)
    );
    assert_eq!(tab_navigation_offset("up", TabDragAxis::Vertical), Some(-1));
    assert_eq!(
        tab_navigation_offset("down", TabDragAxis::Vertical),
        Some(1)
    );
    assert_eq!(tab_navigation_offset("up", TabDragAxis::Horizontal), None);
    assert_eq!(tab_navigation_offset("left", TabDragAxis::Vertical), None);
}

#[test]
fn midpoint_insertion_tracks_both_directions_and_edges() {
    let midpoints = [Some(50.0), Some(120.0), Some(190.0)];
    assert_eq!(midpoint_insertion_index(midpoints, 49.0), 0);
    assert_eq!(midpoint_insertion_index(midpoints, 120.0), 2);
    assert_eq!(midpoint_insertion_index(midpoints, 250.0), 3);
}

#[test]
fn reorder_keeps_stable_ids_and_can_reverse_direction() {
    let mut order = vec![10, 20, 30, 40];
    insert_id_at(&mut order, 10, 3);
    assert_eq!(order, [20, 30, 40, 10]);
    insert_id_at(&mut order, 10, 0);
    assert_eq!(order, [10, 20, 30, 40]);
}

#[test]
fn insertion_grid_stays_fixed_while_tabs_animate() {
    let horizontal_midpoint = stable_slot_midpoint(140.0, 80.0, 40.0);
    let horizontal_midpoint_mid_animation = stable_slot_midpoint(119.0, 80.0, 19.0);
    let vertical_midpoint = stable_slot_midpoint(92.0, 36.0, 12.0);

    assert_eq!(horizontal_midpoint, 140.0);
    assert_eq!(horizontal_midpoint_mid_animation, horizontal_midpoint);
    assert_eq!(vertical_midpoint, 98.0);
}

#[test]
fn neighbor_animation_only_retargets_after_the_slot_changes() {
    assert_eq!(animation_retarget_offset(false, 24.0), None);
    assert_eq!(animation_retarget_offset(true, 24.0), Some(24.0));
    assert_eq!(animation_retarget_offset(true, 0.05), None);
}

#[test]
fn old_orientation_offsets_do_not_leak_into_the_new_layout() {
    let animation = TabPositionAnimation {
        axis: TabDragAxis::Horizontal,
        tween: super::super::super::Tween::at(24.0),
    };
    assert!(tab_position_animation_offset(Some(&animation), TabDragAxis::Horizontal) > 0.0);
    assert_eq!(
        tab_position_animation_offset(Some(&animation), TabDragAxis::Vertical),
        0.0
    );
}

#[test]
fn transfer_plan_preserves_identity_and_rejects_duplicates() {
    let plan = plan_tab_transfer(&[1, 2, 3], &[7, 8], 2, 2, 1).unwrap();
    assert_eq!(plan.source_index, 1);
    assert_eq!(plan.target_index, 1);
    assert_eq!(plan.target_id, 2);

    assert!(plan_tab_transfer(&[1, 2], &[2, 8], 1, 2, 0).is_none());
    assert!(plan_tab_transfer(&[1, 2, 2], &[8], 2, 3, 0).is_none());
    assert!(plan_tab_transfer(&[1, 2], &[8], 3, 3, 0).is_none());
}

#[test]
fn transfer_plan_clamps_to_the_target_end() {
    let plan = plan_tab_transfer(&[4], &[7, 8], 4, 4, 9).unwrap();
    assert_eq!(plan.target_index, 2);
}

#[test]
fn another_window_in_the_drag_path_remains_a_drop_target() {
    let bounds = gpui::Bounds::new(
        gpui::point(gpui::px(100.0), gpui::px(100.0)),
        gpui::size(gpui::px(200.0), gpui::px(200.0)),
    );
    assert!(is_over_or_moving_toward_bounds(
        bounds,
        gpui::point(gpui::px(120.0), gpui::px(120.0)),
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
    ));
    assert!(is_over_or_moving_toward_bounds(
        bounds,
        gpui::point(gpui::px(80.0), gpui::px(150.0)),
        gpui::point(gpui::px(40.0), gpui::px(150.0)),
    ));
    assert!(!is_over_or_moving_toward_bounds(
        bounds,
        gpui::point(gpui::px(40.0), gpui::px(150.0)),
        gpui::point(gpui::px(80.0), gpui::px(150.0)),
    ));
}

#[test]
fn converts_live_global_pointer_to_target_content_coordinates() {
    let global = gpui::point(gpui::px(540.0), gpui::px(390.0));
    let content_origin = gpui::point(gpui::px(500.0), gpui::px(300.0));

    assert_eq!(
        super::content_local_position(global, content_origin),
        gpui::point(gpui::px(40.0), gpui::px(90.0))
    );
}

#[test]
fn external_drop_target_prefers_favourites_then_tabs_then_window_body() {
    let favourites = gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(40.0), gpui::px(40.0)),
    );
    let strip = gpui::Bounds::new(
        gpui::point(gpui::px(20.0), gpui::px(0.0)),
        gpui::size(gpui::px(60.0), gpui::px(40.0)),
    );
    let viewport = gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(200.0), gpui::px(200.0)),
    );

    assert_eq!(
        external_drop_target_kind(
            gpui::point(gpui::px(25.0), gpui::px(20.0)),
            Some(favourites),
            None,
            Some(strip),
            viewport,
        ),
        Some(ExternalDropTargetKind::Favourites)
    );
    assert_eq!(
        external_drop_target_kind(
            gpui::point(gpui::px(70.0), gpui::px(20.0)),
            None,
            None,
            Some(strip),
            viewport,
        ),
        Some(ExternalDropTargetKind::TabStrip)
    );
    assert_eq!(
        external_drop_target_kind(
            gpui::point(gpui::px(150.0), gpui::px(150.0)),
            None,
            None,
            Some(strip),
            viewport,
        ),
        Some(ExternalDropTargetKind::WindowBody)
    );
}
