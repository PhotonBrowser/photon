//! The browser window: its tabs, chrome, menus and page area.

mod actions;
#[cfg_attr(feature = "ui-only", allow(dead_code))]
mod alerts;
mod app;
mod automation;
mod background_noise;
mod chrome;
mod command_palette;
mod commands;
mod content;
mod context_menu;
mod favourites;
mod find;
mod keyboard_shortcuts;
mod menu;
#[cfg(not(feature = "ui-only"))]
mod open_urls;
mod page_menu;
mod peek;
mod pinned;
mod popup_window;
mod popups;
mod registry;
mod session;
mod side_dock;
mod sleeping;
mod spaces;
mod tab_items;
mod tab_menu;
mod tab_summary;
mod tabs;
mod title;
mod workspace;
mod zoom;

use crate::platform::MenuBarState;
use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Pixels, Point, Render,
    ScrollHandle, Subscription, Task, WeakEntity, Window, WindowHandle, div, prelude::*,
};
use photon_core::{BrowserCommand, TabSleepMode, WindowBackgroundStyle};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::super::app_update::update_app;
use super::super::engine::EngineRuntime;
use super::super::trace;
use super::super::window_observer::WindowObserver;
use super::super::window_settings;
use super::command_palette::CommandPalette;
use super::find_bar::FindBar;
use super::history::BrowsingHistory;
use super::js_dialog::DialogHost;
use super::motion::{Presence, Tween, TweenCurve};
use super::omnibox::{Omnibox, OmniboxEvent};
use super::pages::find_page;
use super::settings::Settings;
use super::shortcuts_dialog::ShortcutsDialog;
use super::tabs::{RevealedIcon, TabDragAxis, TabIcon};
use super::theme::{ThemeColors, palette};
use alerts::{AlertCenter, EngineNotice, NoticeExpiry};
use content::{BrowserTab, TabContent, create_webview};
use peek::PeekState;
use popups::PendingPopup;
use registry::BrowserWindowRegistry;
use side_dock::SidebarState;
use spaces::SpacesState;
use tabs::ClosedTab;

pub use alerts::show_alert;
pub use app::run;

struct BrowserWindow {
    tabs: Vec<BrowserTab>,
    /// One per tab, in `tabs` order: follows its web view or page.
    tab_subscriptions: Vec<Option<Subscription>>,
    /// The New Tab page shown while the window has no tabs.
    empty_page: Option<TabContent>,
    _empty_page_subscription: Option<Subscription>,
    /// Rebuilds tab and favourite icons after an asynchronous disk load.
    _history_subscription: Subscription,
    /// Re-renders this window when its transient notice changes.
    _alert_subscription: Subscription,
    /// The GPUI window ID used to target notices to this window.
    window_id: u64,
    /// The sidebar, cached so Engine frames repaint only the page.
    chrome: Entity<BrowserChrome>,
    /// Menus and dialogs over the window, cached for the same reason.
    overlays: Entity<BrowserOverlays>,
    /// Where the sidebar is, in the vertical layout.
    sidebar: SidebarState,
    /// The space shown, switching between spaces, and the space dialog.
    spaces: SpacesState,
    tab_focus_handles: Vec<FocusHandle>,
    /// Closes today's tabs left unused, when the setting asks.
    _archive_task: Task<()>,
    /// Wakes the deadline scheduler when settings or tabs change.
    tab_sleep_waker: sleeping::TabSleepWaker,
    /// Sleeps inactive pages and checks for system memory pressure.
    _tab_sleep_task: Task<()>,
    _memory_pressure_monitor: sleeping::MemoryPressureMonitor,
    tab_sleep_mode: TabSleepMode,
    active_tab: Option<usize>,
    /// The active tab drag, its measured geometry, and the temporary motion.
    tab_drag_layout: Option<TabDragLayout>,
    tab_geometry: HashMap<u64, Bounds<Pixels>>,
    tab_container_bounds: Option<Bounds<Pixels>>,
    tab_strip_scroll: ScrollHandle,
    tab_auto_scroll_at: Option<Instant>,
    favourites_bounds: Option<Bounds<Pixels>>,
    favourites_drag_hover: Option<(AnyWindowHandle, u64)>,
    incoming_tab_drag: Option<IncomingTabDrag>,
    tab_position_animations: HashMap<u64, TabPositionAnimation>,
    suppress_tab_click_until: Option<Instant>,
    omnibox: Entity<Omnibox>,
    _omnibox_subscription: Subscription,
    runtime: Rc<EngineRuntime>,
    find_bar_presence: Presence<Entity<FindBar>>,
    /// A link previewed over the page.
    peek: PeekState,
    /// The keyboard shortcuts dialog, while open.
    shortcuts_dialog: Option<(Entity<ShortcutsDialog>, Subscription)>,
    shortcuts_dialog_presence: Presence<Entity<ShortcutsDialog>>,
    /// The command palette, while open.
    command_palette: Option<Entity<CommandPalette>>,
    command_palette_presence: Presence<Entity<CommandPalette>>,
    _command_palette_subscription: Option<Subscription>,
    notice_presence: Presence<alerts::Notice>,
    /// Pages of closed tabs, most recent last, for reopening.
    closed_tabs: Vec<ClosedTab>,
    /// The loading spinners' animation step, shared by every loading tab.
    spinner_step: usize,
    /// Whether a timer is advancing `spinner_step`.
    spinner_running: bool,
    /// The open find bar, for the active tab.
    find_bar: Option<(Entity<FindBar>, Subscription)>,
    /// The active tab's open JavaScript dialog.
    dialog: DialogHost,
    pending_popups: VecDeque<PendingPopup>,
    /// Keeps the pop-up confirmation drawn while it fades away.
    popup_presence: Presence<String>,
    popup_confirmation_focus: FocusHandle,
    /// The title last given to the window, so it changes only with the page.
    window_title: String,
    /// Whether the window was notified since it last followed its state in
    /// the title, menu bar and session. A page frame alone redraws the window
    /// without notifying it, and changes none of them.
    state_changed: bool,
    /// Holds focus when nothing in the window does, such as after a click
    /// on an empty page, so the browser's shortcuts still reach it.
    root_focus: FocusHandle,
    /// The last Engine service stop or restart, shown in a chip.
    engine_notice: Option<EngineNotice>,
    /// When the zoom chip was last shown, after a zoom change.
    zoom_shown_at: Option<Instant>,
    /// Redraws when a recovery chip expires.
    notice_expiry: NoticeExpiry,
    /// Invalidates expiry tasks after a newer notice replaces the current one.
    notice_expiry_generation: u64,
    /// The icon each tab last revealed and when, so an icon animates in once
    /// rather than whenever the tab redraws it after a spinner.
    revealed_icons: RefCell<HashMap<u64, (RevealedIcon, Instant)>>,
    /// Tabs that should keep their entry wrapper mounted so GPUI owns the full
    /// animation lifecycle and can retarget it smoothly when they close.
    tab_entry_animations: RefCell<HashSet<u64>>,
    /// The latest rendered appearance of each tab, frozen while it closes.
    tab_visuals: RefCell<HashMap<u64, (String, TabIcon)>>,
    /// Tabs that remain mounted while their actual element animates out.
    closing_tabs: HashSet<u64>,
    /// Which closing tabs were active when their exit began.
    closing_tab_active: HashSet<u64>,
    /// Their original layout extents, used to size their absolute exit visuals.
    closing_tab_extents: HashMap<u64, f32>,
    /// The gap removed with each closing tab's zero-size layout slot.
    closing_tab_gaps: HashMap<u64, f32>,
    /// Finalizes each close after its spring has settled.
    tab_close_tasks: HashMap<u64, Task<()>>,
    /// Whether any part of the window is on screen. Engine renders the active
    /// tab only while it is, since GPUI-CE stops drawing an occluded window.
    window_visible: bool,
    window_background_style: WindowBackgroundStyle,
    _appearance_subscription: Subscription,
    /// Redraws as the window comes to the front, so the menu bar follows it.
    _activation_subscription: Subscription,
    _settings_subscription: Subscription,
    _chrome_subscription: Subscription,
    _window_observer: Option<WindowObserver>,
    /// Removes this window from the app registry and saved session on release.
    _window_release: Subscription,
}

impl BrowserWindow {
    pub(super) fn update_menu_bar(&self, window: &Window, cx: &mut App) {
        if !window.is_window_active() {
            return;
        }
        let state = MenuBarState {
            commands: self.command_availability(cx),
            sidebar_icons: self.sidebar.docked() && self.sidebar.collapsed(),
            zen: self.sidebar.zen(),
            loading: self
                .active_webview()
                .is_some_and(|webview| webview.read(cx).state.loading),
        };
        crate::platform::app_menus::update_menu_bar(state, cx);
    }
}

struct TabDragLayout {
    id: u64,
    source_window: AnyWindowHandle,
    axis: TabDragAxis,
    pointer_start: Point<Pixels>,
    pointer_position: Point<Pixels>,
    grab_offset: Point<Pixels>,
    tab_bounds: Bounds<Pixels>,
    pinned: bool,
    phase: TabDragPhase,
    visual_offset: f32,
    target_window: Option<AnyWindowHandle>,
}

#[derive(Clone)]
struct IncomingTabDrag {
    source: AnyWindowHandle,
    id: u64,
    axis: TabDragAxis,
    pointer_position: Point<Pixels>,
    before_id: Option<u64>,
    after_id: Option<u64>,
    size: f32,
    pinned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TabDragPhase {
    PendingDrag,
    Reordering,
    DraggingOverFavourites,
    OutsideTarget,
    CrossWindowTargeting,
    Completing,
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
struct TabPositionAnimation {
    axis: TabDragAxis,
    tween: Tween,
}

impl TabPositionAnimation {
    fn new(
        axis: TabDragAxis,
        from: f32,
        duration: Duration,
        settings: &super::motion::MotionConfig,
    ) -> Self {
        let mut tween = Tween::at_with_curve(from, TweenCurve::SmoothStep);
        tween.animate_to_for(0.0, duration, settings);
        Self { axis, tween }
    }

    fn value(self) -> f32 {
        self.tween.value()
    }

    fn is_running(self) -> bool {
        self.tween.is_running()
    }
}

fn tab_position_animation_offset(
    animation: Option<TabPositionAnimation>,
    axis: TabDragAxis,
) -> f32 {
    animation
        .filter(|animation| animation.axis == axis)
        .map_or(0.0, TabPositionAnimation::value)
}

/// The tab strip and toolbar, or the sidebar. A cached view re-renders only when it is
/// notified, so `BrowserWindow` notifies it whenever the window or a tab's page
/// state changes; Engine frames, which notify only the page view, leave it be.
struct BrowserChrome {
    browser: WeakEntity<BrowserWindow>,
}

impl Render for BrowserChrome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.browser
            .update(cx, |browser, cx| browser.render_chrome(window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

/// What floats over the whole window: the space dialog, a JavaScript dialog,
/// menus and the pop-up prompt. Cached like the chrome, so Engine frames do not
/// rebuild it.
struct BrowserOverlays {
    browser: WeakEntity<BrowserWindow>,
}

impl Render for BrowserOverlays {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.browser
            .update(cx, |browser, cx| browser.render_overlays(window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl BrowserWindow {
    fn palette(&self, window: &Window, cx: &App) -> ThemeColors {
        palette(window, cx)
    }

    /// Redraws the chrome and every tab for changed settings, such as the theme.
    fn settings_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() && self.tab_axis(cx) == TabDragAxis::Horizontal {
            self.open_tab(window, cx);
        }
        let tab_sleep_mode = Settings::get(cx).tab_sleep_mode;
        if tab_sleep_mode != self.tab_sleep_mode {
            self.tab_sleep_mode = tab_sleep_mode;
            self.sleep_idle_tabs(window, cx);
        }
        let background_style = Settings::get(cx).window_background_style;
        if background_style != self.window_background_style {
            window_settings::apply_background_style(window, background_style);
            self.window_background_style = background_style;
        }
        for webview in self.tabs.iter().filter_map(|tab| tab.content.webview()) {
            webview.update(cx, |_, cx| cx.notify());
        }
        self.omnibox.update(cx, |_, cx| cx.notify());
        self.wake_tab_sleep_scheduler();
        self.animate_spinner(cx);
        cx.notify();
    }

    fn window_changed(&mut self, visible: bool, display_changed: bool, cx: &mut Context<Self>) {
        trace(format_args!(
            "window visible={visible} display-changed={display_changed}"
        ));
        crate::platform::automation_lifecycle_trace(format_args!(
            "window_visibility_changed visible={visible} display_changed={display_changed} tabs={} active_tab={:?}",
            self.tabs.len(),
            self.active_tab
        ));
        if self.window_visible != visible {
            self.window_visible = visible;
            if let Some(webview) = self.active_webview() {
                webview.update(cx, |view, _| view.session.set_visible(visible));
            }
        }
        if display_changed {
            for tab in self.tabs.iter().filter_map(|tab| tab.content.webview()) {
                tab.update(cx, |view, cx| {
                    view.session.forget_display();
                    cx.notify();
                });
            }
        }
    }

    fn dispatch_command(
        &mut self,
        command: BrowserCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            BrowserCommand::NewTab => self.open_tab(window, cx),
            BrowserCommand::NewWindow => {
                if let Err(error) = open_browser_window(
                    self.runtime.clone(),
                    app::launch_address(),
                    false,
                    false,
                    cx,
                ) {
                    trace(format_args!("new window: {error:#}"));
                }
            }
            BrowserCommand::Navigate(url) => self.navigate_active(&url, window, cx),
            command => {
                if let Some(webview) = self.active_webview()
                    && let Err(error) = webview.update(cx, |view, cx| {
                        let result = view.execute(command, cx);
                        cx.notify();
                        result
                    })
                {
                    trace(format_args!("browser command: {error:#}"));
                }
            }
        }
        cx.notify();
    }
}

impl BrowserWindow {
    /// Everything that floats over the window, drawn by `BrowserOverlays`.
    fn render_overlays(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette(window, cx);
        let dialog = self.dialog.overlay(cx);
        div()
            .size_full()
            .children(self.peek_overlay(palette, cx))
            .children(self.space_editor_overlay(palette, cx))
            .children(self.shortcuts_dialog_overlay(palette, cx))
            .children(self.command_palette_overlay(window, cx))
            // A JavaScript dialog is modal to the whole window.
            .children(dialog)
            .children(self.popup_confirmation(palette, cx))
            .into_any_element()
    }
}

/// Opens a browser window showing `startup_address`, or a New Tab page when
/// the horizontal layout needs a tab. Session restoration fills tabs afterward.
fn open_browser_window(
    runtime: Rc<EngineRuntime>,
    startup_address: Option<String>,
    restore_pinned_tabs: bool,
    restoring_session: bool,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<BrowserWindow>> {
    // A page's address opens that page; any other address a web page.
    let startup_page = startup_address.as_deref().and_then(find_page);
    let initial_webview = startup_address
        .as_deref()
        .filter(|_| startup_page.is_none())
        .and_then(|address| create_webview(cx, runtime.clone(), address));
    let animations = Settings::get(cx).animations;
    let options = window_settings::options(cx);
    let handle = cx.open_window(options, move |window, cx| {
        window_settings::configure_native_controls(window, &animations);
        window.set_window_title(photon_brand::NAME);
        let omnibox = cx.new(|cx| Omnibox::new(window, cx));
        cx.new(move |cx: &mut Context<BrowserWindow>| {
            let appearance_subscription =
                cx.observe_window_appearance(window, |_, _, cx| cx.notify());
            let activation_subscription =
                cx.observe_window_activation(window, |_, _, cx| cx.notify());
            // Follow settings changed here, in a settings page, or in another
            // window, including the space shown.
            let settings_subscription =
                cx.observe_global_in::<Settings>(window, |this: &mut BrowserWindow, window, cx| {
                    this.follow_space(window, cx);
                    this.settings_changed(window, cx);
                });
            let history_subscription = cx.observe_global::<BrowsingHistory>(|_, cx| cx.notify());
            if !cx.has_global::<AlertCenter>() {
                cx.set_global(AlertCenter::default());
            }
            let alert_subscription = cx.observe_global::<AlertCenter>(|_, cx| cx.notify());
            let browser = cx.entity().downgrade();
            let chrome = cx.new(|_| BrowserChrome {
                browser: browser.clone(),
            });
            let overlays = cx.new(|_| BrowserOverlays { browser });
            // Whatever re-renders the window's own state re-renders the chrome
            // and the overlays too.
            let chrome_subscription = cx.observe_self(|this, cx| {
                this.state_changed = true;
                this.chrome.update(cx, |_, cx| cx.notify());
                this.overlays.update(cx, |_, cx| cx.notify());
            });
            let omnibox_subscription = cx.subscribe_in(
                &omnibox,
                window,
                |this, _, event: &OmniboxEvent, window, cx| match event {
                    OmniboxEvent::Navigate(url) => this.navigate_active(url, window, cx),
                },
            );
            let this = cx.entity().downgrade();
            let app = cx.to_async();
            let window_observer = WindowObserver::new(window, move |visible, display_changed| {
                let this = this.clone();
                app.spawn(async move |cx| {
                    update_app(cx, |cx| {
                        this.update(cx, |this: &mut BrowserWindow, cx| {
                            this.window_changed(visible, display_changed, cx)
                        })
                        .ok();
                    })
                    .await;
                })
                .detach();
            });
            let registry_window_id = window.window_handle().window_id().as_u64();
            let (tab_sleep_waker, tab_sleep_task) =
                BrowserWindow::sleep_inactive_tabs_regularly(window, cx);
            let memory_pressure_monitor =
                sleeping::MemoryPressureMonitor::new(tab_sleep_waker.clone());
            let mut browser = BrowserWindow {
                tabs: Vec::new(),
                tab_subscriptions: Vec::new(),
                empty_page: None,
                _empty_page_subscription: None,
                _history_subscription: history_subscription,
                _alert_subscription: alert_subscription,
                window_id: registry_window_id,
                chrome,
                overlays,
                sidebar: SidebarState::new(cx),
                spaces: SpacesState::new(Settings::get(cx).spaces.active().id),
                tab_focus_handles: Vec::new(),
                _archive_task: BrowserWindow::archive_unused_tabs_regularly(window, cx),
                tab_sleep_waker,
                _tab_sleep_task: tab_sleep_task,
                _memory_pressure_monitor: memory_pressure_monitor,
                tab_sleep_mode: Settings::get(cx).tab_sleep_mode,
                active_tab: None,
                tab_drag_layout: None,
                tab_geometry: HashMap::new(),
                tab_container_bounds: None,
                tab_strip_scroll: ScrollHandle::new(),
                tab_auto_scroll_at: None,
                favourites_bounds: None,
                favourites_drag_hover: None,
                incoming_tab_drag: None,
                tab_position_animations: HashMap::new(),
                suppress_tab_click_until: None,
                omnibox,
                _omnibox_subscription: omnibox_subscription,
                runtime,
                find_bar_presence: Presence::default(),
                peek: PeekState::default(),
                shortcuts_dialog: None,
                shortcuts_dialog_presence: Presence::default(),
                command_palette: None,
                command_palette_presence: Presence::default(),
                _command_palette_subscription: None,
                notice_presence: Presence::default(),
                closed_tabs: Vec::new(),
                spinner_step: 0,
                spinner_running: false,
                find_bar: None,
                dialog: DialogHost::default(),
                pending_popups: VecDeque::new(),
                popup_presence: Presence::default(),
                popup_confirmation_focus: cx.focus_handle(),
                root_focus: cx.focus_handle(),
                window_title: photon_brand::NAME.to_owned(),
                state_changed: true,
                engine_notice: None,
                notice_expiry: None,
                notice_expiry_generation: 0,
                zoom_shown_at: None,
                revealed_icons: RefCell::default(),
                tab_entry_animations: RefCell::default(),
                tab_visuals: RefCell::default(),
                closing_tabs: HashSet::new(),
                closing_tab_active: HashSet::new(),
                closing_tab_extents: HashMap::new(),
                closing_tab_gaps: HashMap::new(),
                tab_close_tasks: HashMap::new(),
                window_visible: true,
                _appearance_subscription: appearance_subscription,
                _activation_subscription: activation_subscription,
                _settings_subscription: settings_subscription,
                _chrome_subscription: chrome_subscription,
                _window_observer: window_observer,
                window_background_style: Settings::get(cx).window_background_style,
                _window_release: {
                    let key = cx.entity_id().as_u64();
                    cx.on_release(move |this, cx| {
                        crate::platform::automation_lifecycle_trace(format_args!(
                            "window_release id={} tabs={} active_tab={:?} visible={}",
                            registry_window_id,
                            this.tabs.len(),
                            this.active_tab,
                            this.window_visible
                        ));
                        BrowserWindow::forget_session(key, cx);
                        AlertCenter::remove_window(registry_window_id, cx);
                        BrowserWindowRegistry::unregister_window(registry_window_id, cx);
                    })
                },
            };
            match (initial_webview, startup_page) {
                (Some(webview), _) => {
                    let content = TabContent::Web(webview);
                    browser.insert_content(0, content, true, window, cx);
                }
                (None, Some(page)) => browser.insert_page(0, page, window, cx),
                (None, None) => {}
            }
            if restore_pinned_tabs {
                browser.restore_pinned_tabs(window, cx);
            }
            if browser.active_tab.is_none() && !browser.tabs.is_empty() {
                browser.activate_tab(0, true, window, cx);
            }
            if browser.tabs.is_empty() {
                if !restoring_session && browser.tab_axis(cx) == TabDragAxis::Horizontal {
                    browser.open_tab(window, cx);
                } else {
                    browser.show_empty_new_tab_page(window, cx);
                }
            }
            browser
        })
    })?;
    let handle = gpui::AnyWindowHandle::from(handle);
    cx.global_mut::<BrowserWindowRegistry>()
        .register(handle.window_id().as_u64());
    let handle = handle
        .downcast::<BrowserWindow>()
        .expect("browser window registry received a different window type");
    Ok(handle)
}
