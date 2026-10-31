//! Browser preferences that are independent of the shell and web engine.

use serde::{Deserialize, Serialize};

use super::new_tab::NewTabSettings;
use super::sidebar::SidebarSettings;
use super::spaces::Spaces;
use super::workspace::WorkspaceSettings;
use crate::tab_sleep::TabSleepMode;

/// Maximum selectable strength for the window background's static noise.
pub const BACKGROUND_NOISE_INTENSITY_MAX: u8 = 20;

/// The selected appearance for pages, web content, menus and context menus.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// How much of the desktop shows through the browser's surfaces.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transparency {
    /// Solid surfaces.
    Off,
    /// A light frosted look that keeps text easy to read.
    #[default]
    Subtle,
    /// More of the desktop shows through.
    Clear,
}

/// Native material used behind transparent window content.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowBackgroundStyle {
    /// Apple's adaptive material on supported macOS versions, with blur on older systems.
    #[default]
    LiquidGlass,
    /// A standard native blurred backdrop.
    Blurred,
}

/// The colour the window is tinted with, as in Arc and Zen.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowColor {
    /// The system's grey.
    #[default]
    System,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
}

impl WindowColor {
    pub const ALL: [Self; 9] = [
        Self::System,
        Self::Blue,
        Self::Purple,
        Self::Pink,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
        }
    }
}

/// Where the window shows its tabs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabLayout {
    /// A tab strip in the titlebar, over an address toolbar.
    Horizontal,
    /// A sidebar beside the page.
    #[default]
    Vertical,
}

/// Where a new tab is inserted in the vertical tab list.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NewTabPosition {
    /// Insert before the other unpinned tabs.
    #[default]
    Top,
    /// Insert after the last tab in the current space.
    AfterLast,
}

/// How Photon handles pages that ask to open a separate window.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum PopupPolicy {
    #[default]
    Ask,
    Allow,
    Block,
}

/// User-adjustable browser preferences, persisted as one profile document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct BrowserSettings {
    pub theme: ThemeMode,
    /// Whether pages should prefer a dark theme and darken light content.
    pub force_dark_mode: bool,
    pub transparency: Transparency,
    /// Continuous transparency, 0 (opaque) to 100. None follows the named preset.
    pub transparency_percent: Option<u8>,
    pub window_background_style: WindowBackgroundStyle,
    /// Native Liquid Glass behind popovers and menus where the platform has it.
    pub glass_surfaces: bool,
    /// How strongly static noise overlays the window background, from 0 to 20.
    /// Legacy boolean values under `background_noise` are migrated on load.
    #[serde(
        default,
        alias = "background_noise",
        deserialize_with = "deserialize_background_noise_intensity"
    )]
    pub background_noise_intensity: u8,
    pub tab_layout: TabLayout,
    /// Where new tabs enter the vertical tab list.
    pub new_tab_position: NewTabPosition,
    /// Saved workspace presets and the profile default used by spaces without
    /// their own preset assignment.
    pub workspace: WorkspaceSettings,
    pub default_search_engine: String,
    /// Whether the address bar shows a page's full address while idle. When
    /// not, it shows only the site until clicked.
    pub show_full_address: bool,
    pub popup_policy: PopupPolicy,
    pub new_tab: NewTabSettings,
    pub sidebar: SidebarSettings,
    /// Whether closing the last tab closes its window. When not, the window
    /// stays open on a new tab page.
    pub close_window_with_last_tab: bool,
    /// The spaces, each with its own colour and pinned tabs, and the one
    /// shown.
    pub spaces: Spaces,
    /// Whether unpinned tabs left unused for twelve hours are closed.
    pub archive_tabs: bool,
    /// How quickly quiet pages out of view are discarded.
    pub tab_sleep_mode: TabSleepMode,
    /// Whether the browser reopens its windows and tabs from last time.
    pub restore_session: bool,
    /// Seconds to keep the shared browser engine running after the last
    /// window closes. Zero quits immediately.
    pub engine_idle_timeout_seconds: u16,
    /// Whether page scrolling can stretch past the viewport edge where supported.
    pub elastic_scroll: bool,
    /// Whether Photon plays subtle sounds for semantic UI feedback.
    pub interaction_sounds: bool,
    /// Timing and effect strengths for shell animations.
    pub animations: AnimationSettings,
}

/// Tunable motion values. Existing profile files use these defaults when the
/// `animations` object or any of its fields is absent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AnimationSettings {
    /// Whether Photon animates UI changes. Reduced motion from the OS still
    /// takes precedence when this is enabled.
    pub enabled: bool,
    pub quick_duration_ms: u16,
    pub standard_duration_ms: u16,
    pub gentle_duration_ms: u16,
    /// Multiplier for slide distances and starting blur, in percent.
    pub effect_strength_percent: u16,
    /// Native traffic light hover and visibility fade durations.
    pub traffic_light_fade_in_ms: u16,
    pub traffic_light_fade_out_ms: u16,
    /// Delay before a page-load spinner replaces its icon, and the cadence
    /// for the spinner's steps.
    pub loading_spinner_delay_ms: u16,
    pub spinner_step_interval_ms: u16,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            quick_duration_ms: 120,
            standard_duration_ms: 200,
            gentle_duration_ms: 320,
            effect_strength_percent: 100,
            traffic_light_fade_in_ms: 320,
            traffic_light_fade_out_ms: 320,
            loading_spinner_delay_ms: 150,
            spinner_step_interval_ms: 80,
        }
    }
}

impl AnimationSettings {
    /// The effective fade-in duration for native window controls.
    pub fn traffic_light_fade_in_ms(self) -> u16 {
        if self.enabled {
            self.traffic_light_fade_in_ms.min(2_000)
        } else {
            0
        }
    }

    /// The effective fade-out duration for native window controls.
    pub fn traffic_light_fade_out_ms(self) -> u16 {
        if self.enabled {
            self.traffic_light_fade_out_ms.min(2_000)
        } else {
            0
        }
    }

    /// The delay before a page-load spinner appears, capped at two seconds.
    pub fn loading_spinner_delay_ms(self) -> u16 {
        self.loading_spinner_delay_ms.min(2_000)
    }

    /// Spinner cadence, kept between one frame and one second.
    pub fn spinner_step_interval_ms(self) -> u16 {
        self.spinner_step_interval_ms.clamp(16, 1_000)
    }
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            force_dark_mode: false,
            transparency: Transparency::default(),
            transparency_percent: None,
            window_background_style: WindowBackgroundStyle::default(),
            glass_surfaces: true,
            background_noise_intensity: 5,
            tab_layout: TabLayout::Vertical,
            new_tab_position: NewTabPosition::default(),
            workspace: WorkspaceSettings::default(),
            default_search_engine: "google".to_owned(),
            show_full_address: false,
            popup_policy: PopupPolicy::Ask,
            new_tab: NewTabSettings::default(),
            sidebar: SidebarSettings::default(),
            close_window_with_last_tab: true,
            spaces: Spaces::default(),
            archive_tabs: false,
            tab_sleep_mode: TabSleepMode::default(),
            restore_session: true,
            engine_idle_timeout_seconds: 60,
            elastic_scroll: true,
            interaction_sounds: false,
            animations: AnimationSettings::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BackgroundNoiseSetting {
    Intensity(u64),
    Enabled(bool),
}

fn deserialize_background_noise_intensity<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match BackgroundNoiseSetting::deserialize(deserializer)? {
        BackgroundNoiseSetting::Intensity(intensity) => {
            intensity.min(u64::from(BACKGROUND_NOISE_INTENSITY_MAX)) as u8
        }
        // Preserve the old on/off setting's appearance when it was enabled.
        BackgroundNoiseSetting::Enabled(true) => 7,
        BackgroundNoiseSetting::Enabled(false) => 0,
    })
}

impl BrowserSettings {
    /// Returns every preference to its default, keeping what people made
    /// themselves: each space with its favourites and pinned tabs, and the
    /// new tab page's shortcuts. Space colours and dismissed hints reset.
    pub fn reset_preferences(&mut self) {
        let shortcuts = std::mem::take(&mut self.new_tab.pinned);
        let mut spaces = std::mem::take(&mut self.spaces);
        spaces.reset_colors();
        spaces.reset_layout_overrides();
        spaces.reset_favourites_hint_dismissals();
        *self = Self::default();
        self.new_tab.pinned = shortcuts;
        self.spaces = spaces;
    }

    /// Moves profile-wide favourites from an older profile into its active
    /// space. Returns whether the profile needs saving after migration.
    pub fn migrate_legacy_favourites(&mut self) -> bool {
        let favourites = std::mem::take(&mut self.sidebar.favourites);
        if favourites.is_empty() {
            return false;
        }

        let active_space = self.spaces.active().id;
        if let Some(space) = self.spaces.get_mut(active_space) {
            for favourite in favourites {
                space.add_favourite(favourite);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Shortcut;

    #[test]
    fn continuous_transparency_round_trips_and_legacy_profiles_keep_the_preset() {
        let legacy: BrowserSettings = serde_json::from_str(r#"{"transparency":"clear"}"#).unwrap();
        assert_eq!(legacy.transparency_percent, None);
        assert_eq!(legacy.transparency, Transparency::Clear);
        let settings = BrowserSettings {
            transparency_percent: Some(37),
            ..Default::default()
        };
        let saved = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<BrowserSettings>(&saved)
                .unwrap()
                .transparency_percent,
            Some(37)
        );
    }

    #[test]
    fn keeps_the_shared_engine_warm_for_one_minute_by_default() {
        assert_eq!(BrowserSettings::default().engine_idle_timeout_seconds, 60);
    }

    #[test]
    fn force_dark_mode_round_trips_and_defaults_off_for_older_profiles() {
        let older: BrowserSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(!older.force_dark_mode);

        let settings = BrowserSettings {
            force_dark_mode: true,
            ..BrowserSettings::default()
        };
        let saved = serde_json::to_string(&settings).unwrap();
        assert!(
            serde_json::from_str::<BrowserSettings>(&saved)
                .unwrap()
                .force_dark_mode
        );
    }

    #[test]
    fn reset_keeps_saved_sites_only() {
        let site = Shortcut {
            title: "Example".to_owned(),
            url: "https://example.com/".to_owned(),
        };
        let mut settings = BrowserSettings {
            theme: ThemeMode::Dark,
            tab_layout: TabLayout::Vertical,
            ..BrowserSettings::default()
        };
        settings.workspace.set_builtin_layout(TabLayout::Vertical);
        let work = settings
            .spaces
            .add(Some("Work".to_owned()), WindowColor::Teal, true);
        settings.sidebar.width = 400;
        let active_space = settings.spaces.active().id;
        settings
            .spaces
            .get_mut(active_space)
            .unwrap()
            .favourites
            .push(site.clone());
        settings.new_tab.pinned.push(site.clone());
        settings.new_tab.hidden.push("example.org".to_owned());
        let space = settings.spaces.get_mut(work).unwrap();
        space.pinned_tabs.push(site.clone());
        space.layout_override = Some(crate::WorkspaceLayout::vertical());
        settings.archive_tabs = true;
        settings.reset_preferences();

        let mut expected = BrowserSettings::default();
        let active_space = expected.spaces.active().id;
        expected
            .spaces
            .get_mut(active_space)
            .unwrap()
            .favourites
            .push(site.clone());
        expected.new_tab.pinned.push(site.clone());
        let work = expected
            .spaces
            .add(Some("Work".to_owned()), WindowColor::System, false);
        expected
            .spaces
            .get_mut(work)
            .unwrap()
            .pinned_tabs
            .push(site);
        assert_eq!(settings, expected);
    }

    #[test]
    fn migrates_legacy_favourites_into_only_the_active_space() {
        let favourite = Shortcut {
            title: "Example".to_owned(),
            url: "https://example.com/".to_owned(),
        };
        let mut settings = BrowserSettings::default();
        let active = settings
            .spaces
            .add(Some("Work".to_owned()), WindowColor::System, false);
        settings.spaces.activate(active);
        settings.sidebar.favourites.push(favourite.clone());

        assert!(settings.migrate_legacy_favourites());
        assert_eq!(settings.spaces.get(active).unwrap().favourites, [favourite]);
        assert!(settings.spaces.get(1).unwrap().favourites.is_empty());
        assert!(settings.sidebar.favourites.is_empty());
        assert!(!settings.migrate_legacy_favourites());
    }
}
