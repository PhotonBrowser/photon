//! Composes the browser chrome, active page, and window-wide overlays.

use gpui::{
    App, Edges, MouseButton, MouseUpEvent, Render, StyleRefinement, Window, div, prelude::*, px,
};
use photon_core::DockRegion;

use super::super::appearance_preview::AppearancePreview;
use super::super::background_noise;
use super::super::find_bar::FIND_BAR_MOTION;
use super::super::motion::{AnimateIn, mix};
use super::super::titlebar::window_drag_area;
use super::super::workspace_layout::WorkspaceLayout;
use super::super::{metrics, theme::palette};
use super::BrowserWindow;

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.tab_container_bounds = None;
        self.favourites_bounds = None;
        let palette = palette(window, cx);
        let animations = super::super::settings::Settings::motion(cx);
        self.sidebar.begin_frame();
        self.sidebar.settle_if_motion_disabled(&animations);
        if !animations.allows_motion() {
            self.tab_position_animations.clear();
            self.new_tab_button_animation = None;
        } else if self.tab_axis(cx) != super::super::tabs::TabDragAxis::Vertical {
            self.new_tab_button_animation = None;
        }
        let find_bar = self.find_bar_presence.sync(
            self.find_bar.as_ref().map(|(bar, _)| bar.clone()),
            FIND_BAR_MOTION,
            &animations,
            cx,
        );
        let layout = self.layout(cx);
        let sidebar_region = layout.sidebar_region();
        self.sidebar.settle();
        let moving = self.sidebar.is_moving();
        if moving {
            self.request_motion_frame(window, cx);
        }
        self.place_window_controls(window, cx);
        let browser = cx.entity().downgrade();
        if std::mem::take(&mut self.state_changed) {
            self.follow_page_title(window, cx);
            self.update_menu_bar(window, cx);
            self.record_session(cx);
        }
        // Shortcuts dispatch from the focused element, so with none focused
        // they would reach nothing.
        if window.focused(cx).is_none() {
            window.focus(&self.root_focus, cx);
        }
        let page_fullscreen = self.sync_page_fullscreen(window, cx);
        let margins = if page_fullscreen {
            Edges::default()
        } else {
            self.page_margins(&layout, cx)
        };
        let page = div()
            .absolute()
            .inset_0()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .pt(px(margins.top))
            .pr(px(margins.right))
            .pb(px(margins.bottom))
            .pl(px(margins.left))
            .child(self.active_view(palette, cx))
            // The find bar floats in the page's top-right corner.
            .children(find_bar.map(|(bar, transition)| {
                div()
                    .absolute()
                    .top(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(div().child(bar).animate(
                        "find-bar-motion",
                        FIND_BAR_MOTION,
                        transition,
                        &animations,
                    ))
            }))
            // Crash and restart chips sit in the page's bottom-right corner.
            .children(self.notice_chip(palette, cx).map(|chip| {
                div()
                    .absolute()
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .bottom(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(chip)
            }));

        // With no bar above the page, its margin moves the window.
        let top_margin = (!page_fullscreen
            && layout.top_extent() * self.sidebar.chrome_shown() == 0.0)
            .then(|| {
                self.with_browser_menu(
                    window_drag_area(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(px(margins.top)),
                    ),
                    cx,
                )
            });
        let chrome = self
            .chrome
            .clone()
            .cached(StyleRefinement::default().absolute().inset_0());
        let reveal_edge = sidebar_region
            .filter(|_| !page_fullscreen && !self.sidebar.docked())
            .map(|region| self.reveal_edge(region, cx));
        let root = div()
            .track_focus(&self.root_focus)
            .size_full()
            .relative()
            .text_color(gpui::rgb(palette.text_primary))
            .child(
                self.backdrop
                    .clone()
                    .cached(StyleRefinement::default().absolute().inset_0()),
            )
            .child(page)
            .children(top_margin)
            .children((!page_fullscreen).then_some(chrome))
            .children(reveal_edge)
            .on_drag_move(
                cx.listener(|this, event, window, cx| this.drag_sidebar_edge(event, window, cx)),
            )
            .on_mouse_up_all(move |event: &MouseUpEvent, phase, _, window, cx| {
                if phase == gpui::DispatchPhase::Capture && event.button == MouseButton::Left {
                    browser
                        .update(cx, |this, cx| {
                            this.finish_sidebar_resize(cx);
                            this.finish_tab_drag(event.position, window, cx);
                        })
                        .ok();
                }
            })
            .child(
                self.overlays
                    .clone()
                    .cached(StyleRefinement::default().absolute().inset_0()),
            );
        self.on_actions(root, cx)
    }
}

impl BrowserWindow {
    /// Redraws the window next frame to move the sidebar on, without
    /// treating the redraw as a change to the window's state, so the
    /// overlays, menu bar and session are left alone.
    fn request_motion_frame(&self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        let browser = cx.entity().downgrade();
        window.on_next_frame(move |_, cx| {
            browser
                .update(cx, |this, cx| {
                    this.motion_frame = true;
                    cx.notify();
                })
                .ok();
        });
    }

    /// The page's margins inside the window: room for the bars around it
    /// and the sidebar beside it, with a small inset elsewhere. Zen mode
    /// tucks the bars away, leaving the inset in their place.
    fn page_margins(&self, layout: &WorkspaceLayout, cx: &App) -> Edges<f32> {
        let chrome_shown = self.sidebar.chrome_shown();
        let inset = metrics::PAGE_INSET;
        let top_extent = layout.top_extent();
        let top = if top_extent > 0.0 {
            mix(inset, top_extent + metrics::PAGE_CHROME_GAP, chrome_shown)
        } else {
            inset
        };
        let mut margins = Edges {
            top,
            right: inset,
            bottom: layout.bottom_extent() * chrome_shown + inset,
            left: inset,
        };
        if let Some(region) = layout.sidebar_region() {
            // The sidebar brings its own padding, so the page meets it.
            let shown = self.sidebar.shown();
            let side = self.sidebar.docked_width(cx) * shown + mix(inset, 0.0, shown);
            match region {
                DockRegion::Trailing => margins.right = side,
                _ => margins.left = side,
            }
        }
        margins
    }
}

/// Appearance-only repaint owner; live drags do not notify browser policy or sessions.
pub(super) struct WindowBackdrop;
impl Render for WindowBackdrop {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let noise = AppearancePreview::noise(cx);
        div()
            .size_full()
            .relative()
            .bg(palette.window_background())
            .children(palette.gradient_overlay(0.0))
            .children((noise > 0).then(|| background_noise::overlay(noise, 0.0)))
    }
}
