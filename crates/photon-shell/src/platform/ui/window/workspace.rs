//! Composes the browser chrome, active page, and window-wide overlays.

use gpui::{Render, StyleRefinement, Window, div, prelude::*, px};

use super::super::find_bar::FIND_BAR_MOTION;
use super::super::motion::{AnimateIn, mix};
use super::super::titlebar::window_drag_area;
use super::super::workspace_layout::ComponentId;
use super::super::{metrics, theme::palette};
use super::BrowserWindow;
use photon_core::DockRegion;

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let find_bar = self.find_bar_presence.sync(
            self.find_bar.as_ref().map(|(bar, _)| bar.clone()),
            FIND_BAR_MOTION,
            cx,
        );
        let editing = self.workspace_edit.is_some();
        let layout = self.workspace_layout(cx);
        let sidebar_region = layout.sidebar_region();
        let has_sidebar = sidebar_region.is_some();
        self.sidebar.settle();
        if self.sidebar.is_moving() {
            window.request_animation_frame();
        }
        let shown = if editing && has_sidebar {
            1.0
        } else if has_sidebar {
            self.sidebar.shown()
        } else {
            0.0
        };
        let has_top_controls = layout
            .components(DockRegion::Top)
            .any(|component| component.id != ComponentId::TabStrip);
        let has_bottom_tabs = layout
            .components(DockRegion::Bottom)
            .any(|component| component.id == ComponentId::TabStrip);
        let has_bottom_controls = layout
            .components(DockRegion::Bottom)
            .any(|component| component.id != ComponentId::TabStrip);
        let top_extent = layout.top_extent()
            + if editing && has_top_controls {
                metrics::WORKSPACE_CANVAS_GAP
            } else {
                0.0
            };
        let bottom_extent = layout.bottom_extent()
            + if editing && has_bottom_tabs && has_bottom_controls {
                metrics::WORKSPACE_CANVAS_GAP
            } else {
                0.0
            }
            + if editing {
                metrics::WORKSPACE_LIBRARY_HEIGHT
            } else {
                0.0
            };
        let side_width = layout
            .sidebar_extent()
            .map_or_else(|| self.sidebar.width(cx), |width| width as f32)
            * shown;
        let page_top = if top_extent > 0.0 {
            top_extent + metrics::PAGE_CHROME_GAP
        } else if has_sidebar {
            mix(metrics::TITLEBAR_HEIGHT, metrics::PAGE_INSET, shown)
        } else {
            metrics::PAGE_CHROME_GAP
        };
        let page_side_padding = mix(metrics::PAGE_INSET, 0.0, shown);
        let page = div()
            .absolute()
            .inset_0()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .px(px(metrics::PAGE_INSET))
            .pt(px(page_top))
            .pb(px(bottom_extent + metrics::PAGE_INSET))
            .when(sidebar_region == Some(DockRegion::Leading), |page| {
                page.pl(px(side_width + page_side_padding))
            })
            .when(sidebar_region == Some(DockRegion::Trailing), |page| {
                page.pr(px(side_width + page_side_padding))
            })
            .child(self.active_view(palette))
            // The find bar floats in the page's top-right corner.
            .children(find_bar.map(|(bar, transition)| {
                div()
                    .absolute()
                    .top(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(
                        div()
                            .child(bar)
                            .animate("find-bar-motion", FIND_BAR_MOTION, transition),
                    )
            }))
            // Crash and restart chips sit in the page's bottom-right corner.
            .children(self.notice_chip(palette, cx).map(|chip| {
                div()
                    .absolute()
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .bottom(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(chip)
            }))
            .when(editing, |page| {
                page.child(
                    div()
                        .id("workspace-page-blocker")
                        .absolute()
                        .inset_0()
                        .occlude(),
                )
            });

        let blank_titlebar = (!editing && has_sidebar && top_extent == 0.0).then(|| {
            self.with_titlebar_menu(
                window_drag_area(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(px(metrics::TITLEBAR_HEIGHT)),
                ),
                cx,
            )
        });
        let chrome = self
            .chrome
            .clone()
            .cached(StyleRefinement::default().absolute().inset_0());
        let reveal_edge = (!editing)
            .then_some(sidebar_region)
            .flatten()
            .filter(|_| !self.sidebar.visible)
            .map(|region| self.reveal_edge(region, cx));
        let root = div()
            .size_full()
            .relative()
            .bg(palette.window_background())
            .text_color(gpui::rgb(palette.text_primary))
            .child(page)
            .children(blank_titlebar)
            .child(chrome)
            .children(reveal_edge)
            .on_drag_move(
                cx.listener(|this, event, window, cx| this.drag_sidebar_edge(event, window, cx)),
            )
            .child(
                self.overlays
                    .clone()
                    .cached(StyleRefinement::default().absolute().inset_0()),
            );
        self.on_actions(root, cx)
    }
}
