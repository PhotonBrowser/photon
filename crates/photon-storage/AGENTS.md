# photon-storage

Own the on-disk profile: where it lives, the settings and history files, saved
page icons, data usage, and the Engine's website data folder. Keep it plain
file access over `photon-core` types, with no GPUI, platform, or Engine
dependency, so settings and internal pages can use it directly.

Write files atomically, treat unreadable files as empty, version file formats
that may change, and cover each file with a test in a temporary profile.
