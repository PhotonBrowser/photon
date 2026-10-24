//! A live showcase of native AppKit controls hosted in the GPUI element tree.

use gpui::{App, Context, Render, WeakEntity, Window, div, prelude::*, px, rgb};

use crate::platform::macos::appkit::{AppKitButton, AppKitButtonStyle, AppKitTabBar};

use super::super::layout::{h_stack, v_stack};
use super::super::{metrics, theme::ThemeColors, theme::palette};
use super::layout::{group, heading, label, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};

const DEMO_TABS: [(&str, &str); 3] = [
    (
        "Overview",
        "A native NSSegmentedControl supplies selection while GPUI renders this content panel.",
    ),
    (
        "Activity",
        "Selection changes arrive through the AppKit action target and update GPUI state.",
    ),
    (
        "Settings",
        "The tab control and its content have separate owners: AppKit for selection, GPUI for layout and content.",
    ),
];

/// Native AppKit controls arranged and retained by GPUI.
pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "appkit",
    title: "AppKit",
    icon: PageIcon::Logo,
    shows_address: true,
    single_tab: true,
    build: |context, cx| cx.new(|cx| AppKitPage::new(context, cx)).into(),
};

pub(super) struct AppKitPage {
    _context: PageContext,
    tab_bar: AppKitTabBar,
    selected_tab: usize,
    symbol_buttons: Vec<AppKitButton>,
    style_buttons: Vec<AppKitButton>,
    text_button: AppKitButton,
    disabled_button: AppKitButton,
    click_count: usize,
    last_action: &'static str,
}

impl AppKitPage {
    fn new(_context: PageContext, cx: &mut Context<Self>) -> Self {
        let page = cx.entity().downgrade();
        let app = cx.to_async();
        let tab_bar = AppKitTabBar::new(&["Overview", "Activity", "Settings"])
            .expect("AppKit creates a non-empty native tab bar")
            .on_change(app.clone(), tab_handler(page.clone()));
        let symbol_buttons = [
            ("cursorarrow", "Pointer"),
            ("hand.raised", "Pan"),
            ("magnifyingglass", "Zoom"),
            ("crop.rotate", "Crop"),
            ("scope", "Inspect"),
            ("square.on.square", "Duplicate"),
        ]
        .into_iter()
        .filter_map(|(symbol, label)| {
            AppKitButton::symbol(symbol, label).map(|button| {
                button
                    .appearance(AppKitButtonStyle::LiquidGlass)
                    .tooltip(label)
                    .on_click(app.clone(), click_handler(page.clone(), label))
            })
        })
        .collect();

        let style_buttons = [
            ("Liquid Glass", AppKitButtonStyle::LiquidGlass),
            ("Toolbar", AppKitButtonStyle::Toolbar),
            ("Borderless", AppKitButtonStyle::Borderless),
        ]
        .into_iter()
        .filter_map(|(title, style)| {
            AppKitButton::title(title).map(|button| {
                button
                    .appearance(style)
                    .on_click(app.clone(), click_handler(page.clone(), title))
            })
        })
        .collect();

        let text_button = AppKitButton::title("New Tab")
            .expect("AppKit creates native text buttons")
            .appearance(AppKitButtonStyle::LiquidGlass)
            .on_click(app.clone(), click_handler(page.clone(), "New Tab"));
        let disabled_button = AppKitButton::symbol("lock.fill", "Unavailable action")
            .expect("AppKit provides the lock.fill system symbol")
            .appearance(AppKitButtonStyle::LiquidGlass)
            .enabled(false)
            .tooltip("This native control is disabled");

        Self {
            _context,
            tab_bar,
            selected_tab: 0,
            symbol_buttons,
            style_buttons,
            text_button,
            disabled_button,
            click_count: 0,
            last_action: "Choose a native control",
        }
    }

    fn toolbar(&self) -> impl IntoElement {
        h_stack()
            .items_center()
            .gap(px(metrics::MENU_ITEM_GAP))
            .children(self.symbol_buttons.iter().cloned().map(|button| {
                button
                    .w(px(metrics::TOOLBAR_BUTTON_SIZE))
                    .h(px(metrics::TOOLBAR_BUTTON_SIZE))
            }))
    }

    fn style_examples(&self) -> impl IntoElement {
        h_stack()
            .items_center()
            .gap(px(metrics::MENU_ITEM_GAP))
            .children(self.style_buttons.iter().cloned())
    }

    fn states(&self) -> impl IntoElement {
        h_stack()
            .items_center()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(self.text_button.clone())
            .child(self.disabled_button.clone())
    }

    fn tab_content(&self, palette: ThemeColors) -> impl IntoElement {
        let (title, description) = DEMO_TABS[self.selected_tab];
        group(palette).w_full().child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(title)
                .child(secondary_text(description, palette)),
        )
    }

    fn section(
        title: &'static str,
        contents: impl IntoElement,
        palette: ThemeColors,
    ) -> impl IntoElement {
        v_stack()
            .w_full()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(label(title, palette))
            .child(group(palette).w_full().child(contents))
    }
}

impl Render for AppKitPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let interaction_label = if self.click_count == 1 {
            "interaction"
        } else {
            "interactions"
        };
        v_stack()
            .id("appkit-page")
            .size_full()
            .items_center()
            .p(px(metrics::INTERNAL_PAGE_INSET))
            .child(
                v_stack()
                    .w_full()
                    .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
                    .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
                    .child(heading(
                        "appkit-title",
                        "AppKit components",
                        "Real macOS controls, hosted and sized by GPUI.",
                        palette,
                    ))
                    .child(Self::section(
                        "Native tab bar",
                        self.tab_bar.clone().w_full(),
                        palette,
                    ))
                    .child(Self::section(
                        "Selected tab content",
                        self.tab_content(palette),
                        palette,
                    ))
                    .child(Self::section("Symbol toolbar", self.toolbar(), palette))
                    .child(Self::section(
                        "Native button styles",
                        self.style_examples(),
                        palette,
                    ))
                    .child(Self::section("Control states", self.states(), palette))
                    .child(
                        div()
                            .id("appkit-status")
                            .aria_live(gpui::accesskit::Live::Polite)
                            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                            .text_color(rgb(palette.text_secondary))
                            .child(format!(
                                "{} · {} native {}",
                                self.last_action, self.click_count, interaction_label
                            )),
                    )
                    .child(secondary_text(
                        "AppKit draws and handles these native controls. GPUI places each NSView, routes its lifecycle, and updates this page from native actions.",
                        palette,
                    )),
            )
    }
}

fn tab_handler(page: WeakEntity<AppKitPage>) -> impl FnMut(usize, &mut App) + 'static {
    move |index, cx| {
        let Some(page) = page.upgrade() else {
            return;
        };
        page.update(cx, |page, cx| {
            if let Some((title, _)) = DEMO_TABS.get(index) {
                page.selected_tab = index;
                page.last_action = *title;
                page.click_count += 1;
                cx.notify();
            }
        });
    }
}

fn click_handler(
    page: WeakEntity<AppKitPage>,
    action: &'static str,
) -> impl FnMut(&mut App) + 'static {
    move |cx| {
        let Some(page) = page.upgrade() else {
            return;
        };
        page.update(cx, |page, cx| {
            page.click_count += 1;
            page.last_action = action;
            cx.notify();
        });
    }
}
