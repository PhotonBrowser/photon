//! The sidebar's edges: dragging its page-facing edge resizes it, collapses
//! it or snaps it shut, and pointing at the window's outer edge reveals it
//! while hidden.

use gpui::{AnyElement, App, Context, DragMoveEvent, Render, Window, div, prelude::*, px};
use photon_core::{DockRegion, SidebarResize};
use photon_haptics::Feedback;
use std::time::Duration;

use super::super::super::controls::OwnsClicks;
use super::super::super::metrics;
use super::super::super::settings::Settings;
use super::super::super::workspace_layout::WorkspaceLayout;
use super::super::BrowserWindow;
use crate::platform::app_update::update_app;

/// How long the pointer may be away before the revealed sidebar leaves, so
/// crossing from the edge into the sidebar does not close it.
const REVEAL_GRACE: Duration = Duration::from_millis(150);
/// How long a resize rests before its width is saved.
const RESIZE_SAVE_DELAY: Duration = Duration::from_millis(300);

/// The sidebar's edge being dragged.
pub(in super::super) struct SidebarEdgeDrag;

impl Render for SidebarEdgeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Pins an edge element to the window's leading or trailing side, `inset`
/// from it.
fn at_side<E: Styled>(element: E, region: DockRegion, inset: f32) -> E {
    if region == DockRegion::Trailing {
        element.right(px(inset))
    } else {
        element.left(px(inset))
    }
}

impl BrowserWindow {
    /// The handle on the page-facing edge of the sidebar.
    pub(super) fn resize_handle(&self, region: DockRegion, cx: &App) -> AnyElement {
        let layout = WorkspaceLayout::current(cx);
        let handle = div()
            .id("sidebar-resize")
            .absolute()
            .top(px(layout.top_extent()))
            .bottom(px(layout.bottom_extent()))
            .w(px(metrics::SIDEBAR_RESIZE_HANDLE_WIDTH))
            .cursor_col_resize()
            .owns_clicks()
            .on_drag(SidebarEdgeDrag, |_, _, _, cx| cx.new(|_| SidebarEdgeDrag));
        let edge = self.sidebar.docked_width(cx) - metrics::SIDEBAR_RESIZE_HANDLE_WIDTH / 2.0;
        at_side(handle, region, edge).into_any_element()
    }

    /// Follows the sidebar's edge as it is dragged: resizing within limits,
    /// collapsing it to icons or hiding it when dragged narrower, and
    /// showing it again when dragged back out. The width is saved once the
    /// drag rests.
    pub(in super::super) fn drag_sidebar_edge(
        &mut self,
        event: &DragMoveEvent<SidebarEdgeDrag>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let x = event.event.position.x;
        let edge = match WorkspaceLayout::current(cx).sidebar_region() {
            Some(DockRegion::Trailing) => window.viewport_size().width - x,
            _ => x,
        };
        let before = (self.sidebar.docked(), self.sidebar.collapsed());
        match SidebarResize::to(f32::from(edge)) {
            SidebarResize::Hide => self.set_sidebar_visible(false, cx),
            SidebarResize::Collapse => {
                self.set_sidebar_visible(true, cx);
                self.set_sidebar_collapsed(true, cx);
            }
            SidebarResize::Width(width) => {
                self.set_sidebar_visible(true, cx);
                self.set_sidebar_collapsed(false, cx);
                if self.sidebar.dragged_width != Some(width as f32) {
                    self.sidebar.dragged_width = Some(width as f32);
                    self.save_sidebar_width_soon(width, cx);
                    cx.notify();
                }
            }
        }
        // Snapping between hidden, icons and labels taps the trackpad.
        if (self.sidebar.docked(), self.sidebar.collapsed()) != before {
            photon_haptics::play(cx, Feedback::Alignment);
        }
    }

    fn save_sidebar_width_soon(&mut self, width: u32, cx: &mut Context<Self>) {
        let space_id = Settings::get(cx).spaces.active().id;
        self.sidebar._save_width = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESIZE_SAVE_DELAY).await;
            update_app(cx, |cx| {
                this.update(cx, |this, cx| {
                    Settings::update(cx, |settings| {
                        settings.sidebar.width = width;
                        if let Some(layout) = settings
                            .spaces
                            .get_mut(space_id)
                            .and_then(|space| space.layout_override.as_mut())
                        {
                            layout.set_preferred_extent(
                                photon_core::SIDEBAR_TOGGLE_COMPONENT_ID,
                                Some(width as u16),
                            );
                            // Older saved layouts still store the composite sidebar
                            // until a space is edited and saved again.
                            layout.set_preferred_extent(
                                photon_core::SIDEBAR_COMPONENT_ID,
                                Some(width as u16),
                            );
                        }
                    });
                    this.sidebar.dragged_width = None;
                })
                .ok()
            })
            .await;
        }));
    }

    /// The window's outer edge below any top bar, which reveals the hidden
    /// sidebar. It lies in the margin beside the page, so it never covers
    /// the page.
    pub(in super::super) fn reveal_edge(
        &self,
        region: DockRegion,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = WorkspaceLayout::current(cx);
        let chrome_shown = self.sidebar.chrome_shown();
        let edge = div()
            .id("reveal-sidebar")
            .absolute()
            .top(px(layout.top_extent() * chrome_shown))
            .bottom(px(layout.bottom_extent() * chrome_shown))
            .w(px(metrics::PAGE_INSET))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.sidebar.pointer_on_edge = *hovered;
                if !*hovered {
                    this.hide_reveal_when_left(cx);
                } else if this.sidebar.set_revealed(true, cx) {
                    cx.notify();
                }
            }));
        at_side(edge, region, 0.0).into_any_element()
    }

    /// Follows the pointer over the revealed sidebar.
    pub(super) fn revealed_sidebar_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.sidebar.pointer_on_sidebar = hovered;
        if !hovered {
            self.hide_reveal_when_left(cx);
        }
    }

    /// Hides the revealed sidebar shortly, unless the pointer comes back or a
    /// menu opened from it is still open.
    fn hide_reveal_when_left(&mut self, cx: &mut Context<Self>) {
        self.sidebar._hide_check = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REVEAL_GRACE).await;
            update_app(cx, |cx| {
                this.update(cx, |this, cx| {
                    let sidebar = &this.sidebar;
                    if !sidebar.pointer_on_edge
                        && !sidebar.pointer_on_sidebar
                        && this.sidebar.set_revealed(false, cx)
                    {
                        cx.notify();
                    }
                })
                .ok()
            })
            .await;
        }));
    }
}
