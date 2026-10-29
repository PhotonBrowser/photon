//! What the site information popover shows, shared by the native AppKit
//! popover and the GPUI one so both list the same controls, words and states.

/// What the popover shows about the active site.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SiteInfo {
    pub(crate) site: String,
    pub(crate) connection_secure: Option<bool>,
    /// Whether the page may play sound; `None` before it has a page.
    pub(crate) sound_on: Option<bool>,
    /// Whether the site may use the microphone; `None` without a site.
    pub(crate) microphone_on: Option<bool>,
}

/// A square control along the top of the popover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SiteControl {
    Camera,
    Microphone,
    Sound,
    PopOut,
}

impl SiteControl {
    pub(crate) const ALL: [Self; 4] = [Self::Camera, Self::Microphone, Self::Sound, Self::PopOut];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::Microphone => "Microphone",
            Self::Sound => "Sound",
            Self::PopOut => "Pop out view",
        }
    }

    /// Whether the control is on, or `None` while it cannot be changed: the
    /// Engine cannot yet give a site the camera or pop a page out.
    pub(crate) fn state(self, info: &SiteInfo) -> Option<bool> {
        match self {
            Self::Camera | Self::PopOut => None,
            Self::Microphone => info.microphone_on,
            Self::Sound => info.sound_on,
        }
    }

    /// The SF Symbol for the native popover.
    pub(crate) fn symbol(self, on: bool) -> &'static str {
        match (self, on) {
            (Self::Camera, _) => "video.slash",
            (Self::Microphone, true) => "mic.fill",
            (Self::Microphone, false) => "mic.slash",
            (Self::Sound, true) => "speaker.wave.2.fill",
            (Self::Sound, false) => "speaker.slash",
            (Self::PopOut, _) => "pip.exit",
        }
    }
}

/// A row leading to where the site's details are managed. The Engine cannot
/// open these yet, so both popovers show them dimmed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SiteDetail {
    Data,
    Permissions,
}

impl SiteDetail {
    pub(crate) const ALL: [Self; 2] = [Self::Data, Self::Permissions];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Data => "Data and cookies",
            Self::Permissions => "Permissions",
        }
    }

    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Data => "Stored by this site",
            Self::Permissions => "Camera, microphone and more",
        }
    }

    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::Data => "externaldrive",
            Self::Permissions => "hand.raised",
        }
    }
}

/// How the connection reads in the popover's footer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConnectionStatus {
    Secure,
    NotSecure,
    Unknown,
}

impl ConnectionStatus {
    pub(crate) fn of(info: &SiteInfo) -> Self {
        match info.connection_secure {
            Some(true) => Self::Secure,
            Some(false) => Self::NotSecure,
            None => Self::Unknown,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Secure => "Secure",
            Self::NotSecure => "Not secure",
            Self::Unknown => "No details",
        }
    }

    /// The heading of the connection's own page.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Secure => "Connection is secure",
            Self::NotSecure => "Connection is not secure",
            Self::Unknown => "No connection details",
        }
    }

    /// What the connection means for what the person sends the site.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::Secure => {
                "This page was loaded over HTTPS, so what you send this site is encrypted on its way."
            }
            Self::NotSecure => {
                "This page was loaded over HTTP. Don't enter passwords or payment details here: \
                 others on the network could see them."
            }
            Self::Unknown => {
                "This page didn't come from the network, so it has no connection to check."
            }
        }
    }

    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::NotSecure => "lock.open",
            Self::Secure | Self::Unknown => "lock",
        }
    }
}

pub(crate) const TITLE: &str = "Site information";
pub(crate) const CLOSE_LABEL: &str = "Close";
pub(crate) const BACK_LABEL: &str = "Back";
/// Disabled until the Engine reports certificates; see
/// `Documentation/Omnibox.md`.
pub(crate) const VIEW_CERTIFICATE_LABEL: &str = "View certificate";
