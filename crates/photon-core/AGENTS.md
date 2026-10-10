# photon-core

Keep this crate framework-independent. It owns browser state and commands,
shared address normalization, the browsing history and requests to clear
browsing data, browser settings, the new tab page's rules, what the command
bar offers, and the addresses of the browser's own pages. Times are supplied
by callers, so the rules stay independent of clocks and storage.

Settings types may derive serde with defaults so older profiles still load;
saving and loading belong in `photon-storage`. Performance diagnostics belong
in `photon-performance`. Do not add GPUI, platform handles, raw C pointers, or
Ladybird types here. The exported C ABI belongs in `photon-ffi`.
