//! The built-in sidebar feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        Sidebar,
        "sidebar",
        "Sidebar",
        "Vertical tabs, favourites, and sidebar navigation.",
        Navigation,
        Registry,
        NO_REQUIREMENTS,
        &[Command::ToggleSidebar, Command::ToggleSidebarIcons]
    );
