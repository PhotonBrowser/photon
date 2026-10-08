//! GPUI window bootstrap and top-level browser layout.

use gpui::{
    Anchor, App, ClickEvent, Context, Entity, FocusHandle, KeyBinding, KeyDownEvent, MouseButton,
    MouseDownEvent, Point, QuitMode, Render, Subscription, Window, WindowAppearance, anchored, div,
    point, prelude::*, px,
};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_core::BrowserCommand;
use photon_shortcuts::{CloseTab, NewTab, browser_shortcuts};
use std::rc::Rc;
use std::time::Duration;

use super::super::engine::{EngineRuntime, UiWake};
use super::super::window_settings;
use super::PhotonWebView;
use super::crash_alert::crash_alert;
use super::layout::v_stack;
use super::menu::{
    menu_action, menu_checkbox, menu_radio, menu_section, menu_separator, menu_surface,
};
use super::omnibox::{FocusOmnibox, Omnibox};
use super::tabs::{TabItem, tab_strip};
use super::theme::{Palette, ThemePreference, metrics};
use super::titlebar::titlebar;
use super::toolbar::{ClickHandler, address_toolbar as build_address_toolbar};

struct BrowserWindow {
    tabs: Vec<Entity<PhotonWebView>>,
    tab_focus_handles: Vec<FocusHandle>,
    active_tab: usize,
    omnibox: Entity<Omnibox>,
    runtime: Rc<EngineRuntime>,
    theme: ThemePreference,
    open_menu: Option<OpenMenu>,
    _appearance_subscription: Subscription,
}

#[derive(Clone, Copy)]
enum OpenMenu {
    Context(Point<gpui::Pixels>),
    Toolbar(Point<gpui::Pixels>),
}

impl BrowserWindow {
    fn active_webview(&self) -> Entity<PhotonWebView> {
        self.tabs[self.active_tab].clone()
    }

    fn set_theme(&mut self, appearance: WindowAppearance, cx: &mut Context<Self>) {
        self.theme.set(appearance);
        cx.set_window_appearance(Some(appearance));

        for tab in self.tabs.clone() {
            let _ = tab.update(cx, |_, cx| cx.notify());
        }
        let _ = self.omnibox.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        if index != self.active_tab {
            if let Some(previous) = self.tabs.get(self.active_tab) {
                previous.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            self.active_tab = index;
        }
        let webview = self.tabs[index].clone();
        let is_blank_tab = webview.read(cx).is_blank_tab();
        webview.update(cx, |view, cx| {
            view.session.set_visible(true);
            if focus_contents && !is_blank_tab {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_webview(webview, window, cx);
            if focus_contents && is_blank_tab {
                omnibox.focus(window, cx);
            }
        });
        cx.notify();
    }

    fn move_tab_focus(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tab(index, false, window, cx);
        window.focus(&self.tab_focus_handles[index], cx);
    }

    fn open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let webview = create_webview(cx, self.runtime.clone(), self.theme.clone(), None, true);
        let index = self.tabs.len();
        self.tabs.push(webview);
        self.tab_focus_handles
            .push(cx.focus_handle().tab_stop(false));
        self.activate_tab(index, true, window, cx);
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        let removed = self.tabs[index].clone();
        removed.update(cx, |view, _| {
            view.session.set_visible(false);
            view.session.set_focus(false);
        });

        if self.tabs.len() == 1 {
            window.remove_window();
            return;
        }

        let was_active = index == self.active_tab;
        self.tabs.remove(index);
        self.tab_focus_handles.remove(index);
        if index < self.active_tab {
            self.active_tab -= 1;
        } else if was_active {
            self.active_tab = index.min(self.tabs.len() - 1);
        }

        if was_active {
            self.activate_tab(self.active_tab, true, window, cx);
        } else {
            cx.notify();
        }
    }

    fn dispatch_command(
        &mut self,
        command: BrowserCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu = None;
        match command {
            BrowserCommand::NewTab => self.open_tab(window, cx),
            BrowserCommand::NewWindow => {
                if let Err(error) = open_browser_window(
                    self.runtime.clone(),
                    initial_address(),
                    self.theme.clone(),
                    cx,
                ) {
                    super::super::trace(format_args!("new window: {error:#}"));
                }
            }
            BrowserCommand::ToggleDebugInfo => {
                let webview = self.active_webview();
                webview.update(cx, |view, cx| {
                    view.set_debug_info_enabled(!view.debug_info_enabled);
                    cx.notify();
                });
            }
            command => {
                let dismiss_crash_alert = matches!(&command, BrowserCommand::Reload);
                let webview = self.active_webview();
                if let Err(error) = webview.update(cx, |view, cx| {
                    if dismiss_crash_alert {
                        view.crash_alert = false;
                    }
                    let result = view.session.execute(command);
                    cx.notify();
                    result
                }) {
                    super::super::trace(format_args!("browser command: {error:#}"));
                }
            }
        }
        cx.notify();
    }

    fn tab_strip(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(self.theme.appearance(window.appearance()));
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, webview)| {
                let state = webview.read(cx).state.clone();
                let tab_id = format!("browser-tab-{:?}", webview.entity_id());
                let label = if !state.title.trim().is_empty() {
                    state.title
                } else if state.url.is_empty() || state.url == "about:blank" {
                    "New Tab".into()
                } else {
                    state.url
                };
                TabItem {
                    id: tab_id,
                    label,
                    active: index == self.active_tab,
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate_tab(index, true, window, cx);
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.modifiers.modified() {
                                return;
                            }
                            let next_index = match event.keystroke.key.as_str() {
                                "left" => Some((index + this.tabs.len() - 1) % this.tabs.len()),
                                "right" => Some((index + 1) % this.tabs.len()),
                                _ => None,
                            };
                            if let Some(next_index) = next_index {
                                this.move_tab_focus(next_index, window, cx);
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx);
                    })),
                }
            })
            .collect();

        tab_strip(
            tabs,
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_tab(window, cx);
            })),
            palette,
        )
    }

    fn address_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(self.theme.appearance(window.appearance()));
        let state = self.active_webview().read(cx).state.clone();
        let can_reload = !state.url.is_empty() && state.url != "about:blank";
        let loading = state.loading;
        let menu_open = matches!(self.open_menu, Some(OpenMenu::Toolbar(_)));

        build_address_toolbar(
            self.omnibox.clone(),
            state.can_go_back,
            state.can_go_forward,
            can_reload,
            loading,
            menu_open,
            palette,
            Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Back, window, cx);
            })),
            Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Forward, window, cx);
            })),
            Box::new(cx.listener(move |this, _, window, cx| {
                let command = if loading {
                    BrowserCommand::StopLoading
                } else {
                    BrowserCommand::Reload
                };
                this.dispatch_command(command, window, cx);
            })),
            Box::new(cx.listener(|this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.open_menu = match this.open_menu {
                    Some(OpenMenu::Toolbar(_)) => None,
                    _ => Some(OpenMenu::Toolbar(toolbar_menu_anchor(event))),
                };
                cx.notify();
            })),
        )
    }

    fn browser_menu(
        &self,
        appearance: WindowAppearance,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static {
        let debug_info_enabled = self.active_webview().read(cx).debug_info_enabled;
        let is_dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let light_appearance = WindowAppearance::Light;
        let dark_appearance = WindowAppearance::Dark;

        let content = v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(menu_action(
                "menu-new-tab",
                "New Tab",
                0,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewTab, window, cx);
                })),
                palette,
            ))
            .child(menu_action(
                "menu-new-window",
                "New Window",
                1,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewWindow, window, cx);
                })),
                palette,
            ))
            .child(menu_checkbox(
                "menu-debug-info",
                "Debug info",
                2,
                debug_info_enabled,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::ToggleDebugInfo, window, cx);
                })),
                palette,
            ))
            .child(menu_separator(palette))
            .child(menu_section("Theme", palette))
            .child(menu_radio(
                "menu-theme-light",
                "Light",
                3,
                !is_dark,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.set_theme(light_appearance, cx);
                })),
                palette,
            ))
            .child(menu_radio(
                "menu-theme-dark",
                "Dark",
                4,
                is_dark,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.set_theme(dark_appearance, cx);
                })),
                palette,
            ));

        menu_surface(content, palette)
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.theme.appearance(window.appearance());
        let palette = Palette::for_appearance(appearance);
        let mut root = v_stack()
            .size_full()
            .relative()
            .bg(gpui::rgba(palette.window_tint))
            .text_color(gpui::rgb(palette.text))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if f32::from(event.position.y) <= metrics::TITLEBAR_HEIGHT {
                        this.open_menu = Some(OpenMenu::Context(event.position));
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.open_menu.is_some() {
                        this.open_menu = None;
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                    } else if let Some(tab) = this
                        .tabs
                        .iter()
                        .find(|tab| tab.read(cx).crash_alert)
                        .cloned()
                    {
                        tab.update(cx, |view, cx| {
                            view.crash_alert = false;
                            cx.notify();
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &FocusOmnibox, window, cx| {
                this.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                this.dispatch_command(BrowserCommand::NewTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.open_menu = None;
                this.close_tab(this.active_tab, window, cx);
            }))
            .child(titlebar(self.tab_strip(window, cx)))
            .child(self.address_toolbar(window, cx))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .px(px(metrics::PAGE_INSET))
                    .pb(px(metrics::PAGE_INSET))
                    .child(self.active_webview()),
            );
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| tab.read(cx).crash_alert)
            .cloned()
        {
            let reload_tab = tab.clone();
            let dismiss_tab = tab;
            let on_reload: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
                reload_tab.update(cx, |view, cx| {
                    view.crash_alert = false;
                    if let Err(error) = view.session.execute(BrowserCommand::Reload) {
                        eprintln!("Photon Engine: retrying crashed page failed: {error:#}");
                    }
                    cx.notify();
                });
                cx.notify();
            }));
            let on_dismiss: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
                dismiss_tab.update(cx, |view, cx| {
                    view.crash_alert = false;
                    cx.notify();
                });
                cx.notify();
            }));
            root = root.child(crash_alert(palette, on_reload, on_dismiss));
        }
        if let Some(open_menu) = self.open_menu {
            let appearance = self.theme.appearance(window.appearance());
            let palette = Palette::for_appearance(appearance);
            let backdrop = div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.open_menu = None;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.open_menu = None;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
            let menu = self.browser_menu(appearance, palette, cx);
            let (anchor, position) = match open_menu {
                OpenMenu::Context(position) => (Anchor::TopLeft, position),
                OpenMenu::Toolbar(position) => (Anchor::TopRight, position),
            };
            root = root.child(backdrop).child(
                anchored()
                    .anchor(anchor)
                    .position(position)
                    .snap_to_window()
                    .child(menu),
            );
        }
        root
    }
}

fn toolbar_menu_anchor(event: &ClickEvent) -> Point<gpui::Pixels> {
    match event {
        ClickEvent::Keyboard(event) => event.bounds.bottom_right(),
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => {
            let position = event.position();
            let button_center_offset = px(metrics::TOOLBAR_BUTTON_SIZE / 2.0);
            point(
                position.x + button_center_offset,
                position.y + button_center_offset,
            )
        }
    }
}

pub fn run() {
    application().run(|cx: &mut App| {
        // GPUI keeps macOS apps alive after their last window closes by default. Photon
        // has one browser window and owns an Engine runtime, so closing it must quit the
        // app and run the registered Engine/GPU shutdown path.
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
        cx.bind_keys([KeyBinding::new("secondary-l", FocusOmnibox, None)]);
        // Keep browser tab actions and their default bindings in the shortcuts crate.
        cx.bind_keys(browser_shortcuts());
        let runtime = Rc::new(
            EngineRuntime::create()
                .unwrap_or_else(|error| panic!("could not start Photon Engine: {error:#}")),
        );
        quit_after_env_timeout(cx);
        cx.activate(true);
        open_browser_window(runtime, initial_address(), ThemePreference::default(), cx)
            .expect("open GPUI-CE Photon window");
    });
}

fn initial_address() -> Option<String> {
    std::env::var("PHOTON_URL")
        .ok()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
}

fn open_browser_window(
    runtime: Rc<EngineRuntime>,
    startup_address: Option<String>,
    theme: ThemePreference,
    cx: &mut App,
) -> anyhow::Result<()> {
    let is_blank_tab = startup_address.is_none();
    let webview = create_webview(
        cx,
        runtime.clone(),
        theme.clone(),
        startup_address.as_deref(),
        is_blank_tab,
    );
    cx.open_window(window_settings::options(cx), move |window, cx| {
        window.set_window_title("Photon");
        webview.update(cx, |view, cx| {
            view.session.set_visible(true);
            if !is_blank_tab {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        let omnibox = cx.new(|cx| Omnibox::new(webview.clone(), window, cx));
        if is_blank_tab {
            omnibox.update(cx, |omnibox, cx| omnibox.focus(window, cx));
        }
        cx.new(move |cx| {
            let appearance_subscription =
                cx.observe_window_appearance(window, |_, _, cx| cx.notify());
            BrowserWindow {
                tabs: vec![webview],
                tab_focus_handles: vec![cx.focus_handle().tab_stop(true)],
                active_tab: 0,
                omnibox,
                runtime,
                theme,
                open_menu: None,
                _appearance_subscription: appearance_subscription,
            }
        })
    })?;
    Ok(())
}

/// Starts the Engine-backed WebView, wired to redraw on Engine frames and to drain
/// GPU work before the app quits.
fn create_webview(
    cx: &mut App,
    runtime: Rc<EngineRuntime>,
    theme: ThemePreference,
    startup_address: Option<&str>,
    is_blank_tab: bool,
) -> Entity<PhotonWebView> {
    let startup_address = startup_address.map(str::to_owned);
    let webview = cx.new(|cx| {
        let mut webview =
            PhotonWebView::new(cx, runtime, theme, startup_address.as_deref(), is_blank_tab)
                .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"));
        let gpu_activity = webview.session.gpu_activity.clone();
        webview._quit_subscription = Some(cx.on_app_quit(
            move |view: &mut PhotonWebView, _cx: &mut Context<'_, PhotonWebView>| {
                view.prepare_shutdown();
                gpu_activity.wait_until_idle();
                view.session.finish_shutdown();
                async {}
            },
        ));
        webview
    });
    let wake = UiWake {
        app: cx.to_async(),
        webview: webview.downgrade(),
    };
    webview.update(cx, |view, _| view.session.set_ui_wake(wake));
    webview
}

/// Quits after PHOTON_SHUTDOWN_AFTER_SECONDS, for timed shutdown checks.
fn quit_after_env_timeout(cx: &mut App) {
    let Some(seconds) = std::env::var("PHOTON_SHUTDOWN_AFTER_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
    else {
        return;
    };
    let timer = cx.background_executor().timer(Duration::from_secs(seconds));
    cx.spawn(async move |cx| {
        timer.await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}
