//! Composes the browser chrome, active page, and window-wide overlays.

use gpui::{Render, StyleRefinement, Window, div, prelude::*, px};

use super::super::find_bar::FIND_BAR_MOTION;
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, mix};
use super::super::titlebar::window_drag_area;
use super::super::workspace_layout::WorkspaceLayout;
use super::super::{metrics, theme::palette};
use super::BrowserWindow;

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let find_bar = self.find_bar_presence.sync(
            self.find_bar.as_ref().map(|(bar, _)| bar.clone()),
            FIND_BAR_MOTION,
            cx,
        );
        let has_sidebar = WorkspaceLayout::current(cx).has_sidebar();
        self.sidebar.settle();
        if self.sidebar.is_moving() {
            window.request_animation_frame();
        }
        // How far the sidebar is shown beside the page: the page's leading
        // edge follows the sidebar, and its top rises from below the top bar.
        let shown = if has_sidebar {
            self.sidebar.shown()
        } else {
            0.0
        };
        let page = div()
            .relative()
            .flex_1()
            // Keep tall pages inside the page area; they scroll themselves.
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .size_full()
            .px(px(metrics::PAGE_INSET))
            .pb(px(metrics::PAGE_INSET))
            .when(!has_sidebar, |page| page.pt(px(metrics::PAGE_CHROME_GAP)))
            .when(has_sidebar, |page| {
                // Beside the sidebar, its own padding separates the two;
                // with it hidden, the top bar above is gap enough.
                page.pl(px(mix(metrics::PAGE_INSET, 0.0, shown))).pt(px(mix(
                    metrics::TITLEBAR_HEIGHT,
                    metrics::PAGE_INSET,
                    shown,
                )))
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
            }));
        // The horizontal layout stacks chrome above the page. The vertical
        // layout layers the sidebar and makes room for it beside the page.
        let (body, edge) = if has_sidebar {
            let chrome = self.chrome.clone().cached(
                StyleRefinement::default()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            );
            let room = div().flex_shrink_0().w(px(self.sidebar.width(cx) * shown));
            // Empty space across the top moves the window.
            let titlebar = self.with_titlebar_menu(
                window_drag_area(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(px(metrics::TITLEBAR_HEIGHT)),
                ),
                cx,
            );
            let body = div()
                .size_full()
                .relative()
                .child(titlebar)
                .child(h_stack().size_full().child(room).child(page))
                .child(chrome);
            (body, (!self.sidebar.visible).then(|| self.reveal_edge(cx)))
        } else {
            let chrome = self.chrome.clone().cached(
                StyleRefinement::default()
                    .w_full()
                    .h(px(metrics::CHROME_HEIGHT))
                    .flex_shrink_0(),
            );
            (v_stack().size_full().child(chrome).child(page), None)
        };
        let root = div()
            .size_full()
            .relative()
            .bg(palette.window_background())
            .text_color(gpui::rgb(palette.text_primary))
            .child(body)
            .children(edge)
            .on_drag_move(cx.listener(|this, event, _, cx| this.drag_sidebar_edge(event, cx)))
            .child(
                self.overlays
                    .clone()
                    .cached(StyleRefinement::default().absolute().inset_0()),
            );
        self.on_actions(root, cx)
    }
}
