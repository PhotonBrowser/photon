//! What the new tab page shows, and the rules for customising it.

use serde::{Deserialize, Serialize};

use super::history::HistoryEntry;

/// The numbers of shortcuts the new tab page can show.
pub const SHORTCUT_COUNTS: [usize; 5] = [4, 6, 8, 10, 12];
/// How many shortcuts the new tab page shows unless chosen otherwise.
const DEFAULT_SHORTCUT_COUNT: usize = 8;

/// A site on the new tab page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Shortcut {
    pub title: String,
    pub url: String,
}

/// How the new tab page is laid out, as chosen in its customise panel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct NewTabSettings {
    /// Show the Photon logo.
    pub show_logo: bool,
    /// Show shortcuts to sites.
    pub show_shortcuts: bool,
    /// How many shortcuts to show, one of [`SHORTCUT_COUNTS`].
    pub shortcut_count: usize,
    /// Sites pinned to the page, shown first and in this order.
    pub pinned: Vec<Shortcut>,
    /// Sites removed from the page, never suggested again as shortcuts. Older
    /// profiles saved addresses here; each counts as its site.
    pub hidden: Vec<String>,
}

impl Default for NewTabSettings {
    fn default() -> Self {
        Self {
            show_logo: true,
            show_shortcuts: true,
            shortcut_count: DEFAULT_SHORTCUT_COUNT,
            pinned: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

impl NewTabSettings {
    /// How many shortcuts to show: the chosen count, or the nearest allowed
    /// one when a saved count is not.
    pub fn shortcut_limit(&self) -> usize {
        SHORTCUT_COUNTS
            .into_iter()
            .min_by_key(|count| count.abs_diff(self.shortcut_count))
            .unwrap_or(DEFAULT_SHORTCUT_COUNT)
    }

    /// How many shortcuts sit in a row: all of them up to five, otherwise
    /// half, so the page shows at most two rows.
    pub fn shortcut_columns(&self) -> usize {
        let limit = self.shortcut_limit();
        if limit <= 5 { limit } else { limit.div_ceil(2) }
    }

    /// The shortcuts to show: pinned sites, then the most visited sites that
    /// were not removed, one per site, up to [`Self::shortcut_limit`]. A
    /// site's most visited page stands for it.
    pub fn shortcuts(&self, most_visited: &[HistoryEntry]) -> Vec<Shortcut> {
        if !self.show_shortcuts {
            return Vec::new();
        }
        let limit = self.shortcut_limit();
        let mut shortcuts: Vec<Shortcut> = self.pinned.iter().take(limit).cloned().collect();
        let mut sites: Vec<String> = shortcuts.iter().map(|pin| site_name(&pin.url)).collect();
        for entry in most_visited {
            if shortcuts.len() == limit {
                break;
            }
            let site = site_name(&entry.url);
            if self.is_hidden(&site) || sites.contains(&site) {
                continue;
            }
            sites.push(site);
            shortcuts.push(Shortcut {
                title: entry.title.clone(),
                url: entry.url.clone(),
            });
        }
        shortcuts
    }

    /// How much of the history the page needs to fill its shortcuts after
    /// skipping removed sites and repeated pages of one site.
    pub fn history_needed(&self) -> usize {
        self.shortcut_limit() * 4 + self.hidden.len()
    }

    fn is_hidden(&self, site: &str) -> bool {
        self.hidden.iter().any(|hidden| site_name(hidden) == site)
    }

    pub fn is_pinned(&self, url: &str) -> bool {
        self.pinned.iter().any(|pin| pin.url == url)
    }

    /// Pins a site, keeping it on the page whatever is visited.
    pub fn pin(&mut self, shortcut: Shortcut) {
        let site = site_name(&shortcut.url);
        self.hidden.retain(|hidden| site_name(hidden) != site);
        if !self.is_pinned(&shortcut.url) {
            self.pinned.push(shortcut);
        }
    }

    pub fn unpin(&mut self, url: &str) {
        self.pinned.retain(|pin| pin.url != url);
    }

    /// Takes a site off the page: unpins the shortcut and stops suggesting
    /// any of the site's pages.
    pub fn remove(&mut self, url: &str) {
        self.unpin(url);
        let site = site_name(url);
        if !self.is_hidden(&site) {
            self.hidden.push(site);
        }
    }

    /// Whether removed sites can be brought back.
    pub fn has_removed(&self) -> bool {
        !self.hidden.is_empty()
    }

    /// Brings back every removed site.
    pub fn restore_removed(&mut self) {
        self.hidden.clear();
    }
}

/// A site's host without `www.`, such as `github.com`, to name and group
/// its pages.
pub fn site_name(url: &str) -> String {
    let address = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = address.split(['/', '?', '#']).next().unwrap_or(address);
    host.strip_prefix("www.").unwrap_or(host).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visited(url: &str, title: &str) -> HistoryEntry {
        HistoryEntry {
            url: url.to_owned(),
            title: title.to_owned(),
            visit_count: 1,
            last_visit: 1,
        }
    }

    fn urls(shortcuts: &[Shortcut]) -> Vec<&str> {
        shortcuts
            .iter()
            .map(|shortcut| shortcut.url.as_str())
            .collect()
    }

    #[test]
    fn shows_pinned_sites_before_the_most_visited() {
        let mut settings = NewTabSettings::default();
        settings.pin(Shortcut {
            title: "Docs".to_owned(),
            url: "https://docs.example/".to_owned(),
        });
        let most_visited = [
            visited("https://a.example/", "A"),
            visited("https://docs.example/", "Docs"),
        ];
        assert_eq!(
            urls(&settings.shortcuts(&most_visited)),
            ["https://docs.example/", "https://a.example/"]
        );
    }

    #[test]
    fn removing_a_site_hides_it_until_restored() {
        let mut settings = NewTabSettings::default();
        let most_visited = [
            visited("https://a.example/", "A"),
            visited("https://a.example/other", "A other"),
        ];
        settings.remove("https://a.example/");
        assert!(settings.shortcuts(&most_visited).is_empty());
        assert!(settings.has_removed());
        settings.restore_removed();
        assert_eq!(
            urls(&settings.shortcuts(&most_visited)),
            ["https://a.example/"]
        );
    }

    #[test]
    fn hiding_shortcuts_shows_none() {
        let settings = NewTabSettings {
            show_shortcuts: false,
            ..NewTabSettings::default()
        };
        assert!(
            settings
                .shortcuts(&[visited("https://a.example/", "A")])
                .is_empty()
        );
    }

    #[test]
    fn shows_one_shortcut_per_site() {
        let settings = NewTabSettings::default();
        let most_visited = [
            visited("https://github.com/photon", "Photon"),
            visited("https://www.github.com/", "GitHub"),
            visited("https://example.com/", "Example"),
        ];
        assert_eq!(
            urls(&settings.shortcuts(&most_visited)),
            ["https://github.com/photon", "https://example.com/"]
        );
        assert_eq!(site_name("https://www.github.com/a?b"), "github.com");
    }

    #[test]
    fn shows_the_chosen_number_in_at_most_two_rows() {
        let most_visited: Vec<HistoryEntry> = (0..20)
            .map(|index| visited(&format!("https://{index}.example/"), ""))
            .collect();
        for (count, columns) in [(4, 4), (6, 3), (8, 4), (10, 5), (12, 6)] {
            let settings = NewTabSettings {
                shortcut_count: count,
                ..NewTabSettings::default()
            };
            assert_eq!(settings.shortcuts(&most_visited).len(), count);
            assert_eq!(settings.shortcut_columns(), columns);
        }
    }

    #[test]
    fn rounds_a_saved_count_to_an_allowed_one() {
        let settings = NewTabSettings {
            shortcut_count: 9,
            ..NewTabSettings::default()
        };
        assert!(SHORTCUT_COUNTS.contains(&settings.shortcut_limit()));
    }

    #[test]
    fn treats_a_removed_address_as_its_site() {
        let settings = NewTabSettings {
            hidden: vec!["https://www.github.com/old".to_owned()],
            ..NewTabSettings::default()
        };
        assert!(
            settings
                .shortcuts(&[visited("https://github.com/", "GitHub")])
                .is_empty()
        );
    }
}
