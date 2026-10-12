//! The new tab page's customise button and the menu it opens above itself.

use gpui::{Context, KeyDownEvent, Role, deferred, div, prelude::*, px, rgb, rgba};

use super::super::super::controls::{FocusRing, OwnsClicks, choices};
use super::super::super::icons::edit_icon;
use super::super::super::layout::{h_stack, v_stack};
use super::super::super::menu::{
    MENU_MOTION, menu_action, menu_block, menu_heading, menu_separator, menu_switch,
    popover_surface,
};
use super::super::super::motion::Transition;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::SETTINGS;
use super::NewTabPage;
use super::options::LayoutOptions;

impl NewTabPage {
    /// The button in the page's corner, with the customise menu just above
    /// it, right edges aligned, while open.
    pub(super) fn customise_button(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let animations = super::super::super::settings::Settings::get(cx).animations;
        let shown = self.customising.then_some(());
        let menu = self
            .customise_presence
            .sync(shown, MENU_MOTION, &animations, cx)
            .map(|((), transition)| {
                deferred(
                    div()
                        .absolute()
                        .right_0()
                        .bottom(px(metrics::SMALL_BUTTON_HEIGHT + metrics::MENU_ITEM_GAP))
                        .occlude()
                        // A click elsewhere or Escape closes it, as any menu.
                        .when(self.customising, |menu| {
                            menu.on_mouse_down_out(
                                cx.listener(|this, _, _, cx| this.set_customising(false, cx)),
                            )
                            .on_key_down(cx.listener(
                                |this, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.key == "escape"
                                        && !event.keystroke.modifiers.modified()
                                    {
                                        cx.stop_propagation();
                                        this.set_customising(false, cx);
                                        window.focus(&this.customise_focus, cx);
                                    }
                                },
                            ))
                        })
                        .child(self.customise_menu(transition, palette, cx)),
                )
                .priority(1)
            });
        h_stack()
            .id("new-tab-customise")
            .relative()
            .role(Role::Button)
            .aria_label("Customise new tab page")
            .aria_expanded(self.customising)
            .track_focus(&self.customise_focus)
            .tab_index(0)
            .focus_ring(palette)
            .items_center()
            .gap(px(metrics::MENU_ITEM_GAP))
            .h(px(metrics::SMALL_BUTTON_HEIGHT))
            .px(px(metrics::SMALL_BUTTON_HORIZONTAL_PADDING))
            .rounded_full()
            .text_size(px(metrics::SMALL_BUTTON_FONT_SIZE))
            .text_color(rgb(palette.text_secondary))
            .bg(rgba(if self.customising {
                palette.selected_surface
            } else {
                palette.surface
            }))
            .when(!self.customising, |button| {
                button.hover(|style| style.bg(rgba(palette.surface_hover)))
            })
            .owns_clicks()
            // The state when drawn: a click outside the open panel closes it
            // on mouse down, and this click must not open it again.
            .on_click(cx.listener({
                let customising = self.customising;
                move |this, _, _, cx| this.set_customising(!customising, cx)
            }))
            .child(edit_icon(
                palette.text_secondary,
                metrics::ICON_BUTTON_ICON_SIZE,
            ))
            .child("Customise")
            .children(menu)
    }

    /// The layout options as a menu, in the same style as the browser menu.
    fn customise_menu(
        &self,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let options = LayoutOptions::read(cx);
        let mut content = v_stack()
            .w_full()
            .child(menu_switch(
                "new-tab-show-logo",
                "Show logo",
                0,
                options.show_logo,
                options.toggle_logo(),
                palette,
            ))
            .child(menu_switch(
                "new-tab-show-shortcuts",
                "Show shortcuts",
                1,
                options.show_shortcuts,
                options.toggle_shortcuts(),
                palette,
            ));
        if options.show_shortcuts {
            content = content
                .child(menu_heading("Number of shortcuts", palette))
                .child(menu_block(choices(
                    "new-tab-shortcut-count",
                    "Number of shortcuts",
                    LayoutOptions::shortcut_counts(cx),
                    palette,
                )));
        }
        content = content.child(menu_separator(palette));
        if options.has_removed {
            content = content.child(menu_action(
                "new-tab-restore-shortcuts",
                "Restore removed shortcuts",
                2,
                options.restore_removed(),
                palette,
            ));
        }
        content = content.child(menu_action(
            "new-tab-all-settings",
            "All settings…",
            3,
            Box::new(cx.listener(|this, _, _, cx| {
                this.customising = false;
                this.context.open_page(SETTINGS, cx);
            })),
            palette,
        ));
        popover_surface(
            "Customise new tab page",
            Role::Dialog,
            metrics::MENU_WIDTH,
            content,
            transition,
            palette,
            cx,
        )
    }
}
