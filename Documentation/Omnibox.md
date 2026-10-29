# Omnibox

The address field in the titlebar is an omnibox: it takes text and decides what
it means. The framework-free rules live in
[`photon-core/src/omnibox/`](../crates/photon-core/src/omnibox/mod.rs), and
nothing else in Photon re-implements them.

## Rules

| Typed | Resolves to | Kind |
| --- | --- | --- |
| `https://example.com/foo` | opened as written | `Url { kind: Explicit }` |
| `example.com` | `https://example.com/` | `Url { kind: Inferred }` |
| `localhost:3000` | `http://localhost:3000/` | `Url { kind: Local }` |
| `192.168.1.10` | `http://192.168.1.10/` | `Url { kind: Local }` |
| `mailto:person@example.com` | opened as written | `Url { kind: Other }` |
| `photon://settings` | one of the browser's own pages | `Url { kind: Other }` |
| `how to bake bread` | a Google search | `Search` |
| `note: buy milk` | a Google search | `Search` |
| `1.2.3.4.5` | a Google search | `Search` |
| *(nothing)* | `OmniboxError::Empty` | |

A bare host is only a host when it has a dot, no whitespace, valid labels, and a
last label that is either a real IPv4 address or an alphabetic top level domain.
That is why `hello.world` opens and `3.14` searches. A `word:` prefix is a scheme
only when the scheme is known, which keeps `note: buy milk` a search.

Queries are whitespace-collapsed and percent-encoded by the engine, so a space is
`%20` and `&` cannot break out of the parameter.

## Engines

`SearchEngine` is data: an id, a name, and an endpoint with one `{query}`
placeholder. `SearchEngines` is a registry with a settable default, and
`SearchEngines::builtin()` ships Google, DuckDuckGo, Bing, Ecosia and Wikipedia
with Google as the default; the search engine setting chooses another. Another engine is one value:

```rust
let mut engines = SearchEngines::builtin();
engines.register(kagi)?;      // an id that already exists is replaced
engines.set_default("kagi");   // false when the id is unknown
resolve_with("photon browser", &engines);
```

Registration validates the template, so a malformed endpoint is rejected when it
is registered rather than producing an unopenable address when someone presses
Enter.

The browser's own page scheme comes from `photon-brand`; see
[the browser's own pages](Pages.md).

## Suggestions

While you type, the field opens into a panel over the page with the field as
its first row and suggestions under it. `suggest` in
[`suggest.rs`](../crates/photon-core/src/omnibox/suggest.rs) decides them from the
[history](Profile.md#history):

- Visited pages match when every typed word starts a word in their title or
  address. They rank by how well they match and how often they were visited.
- A visited page whose address continues the typed text leads, and the field
  completes it inline, with the added part selected so typing carries on over
  it. Deleting text does not complete it again.
- Otherwise the typed address leads when the text is one, and a search when
  not. The search row stays among the first rows so a word that looks like a
  host can still be searched for.
- Past searches that continue the text follow, with a clock icon.

Rows carry their match ranges, drawn in bold, and addresses show the short
way (no `https://`, `www.` or lone trailing slash) in a link blue. The arrow
keys move through rows and show each in the field; Enter or a click opens
one; a visited page or past search can be forgotten with its close button.
Choosing a search records it in the history.

## Command palette

⌘K or Ctrl+K opens the command palette. Search browser commands by name or
alias, move through available results with the arrow keys, and run the selected
command with Enter. Escape closes it. Available commands follow the current
window, so actions such as Back, Stop Loading, and Reopen Closed Tab appear
when they can run. Keyboard shortcuts appear beside commands that have one.

The command registry lives in
[`photon-core/src/commands.rs`](../crates/photon-core/src/commands.rs); the
palette view and window integration live in
[`ui/command_palette.rs`](../crates/photon-shell/src/platform/ui/command_palette.rs)
and [`ui/window/command_palette.rs`](../crates/photon-shell/src/platform/ui/window/command_palette.rs).
⌘T and “New Tab” controls continue to open a regular new tab.

## Site information

The button at the start of the field opens the active site's information: the
site's controls (camera, microphone, sound and pop-out), its data and
permissions, and its connection. Pressing the connection pill opens the
connection's own page, which explains what the connection means for what the
person sends the site.

On macOS the popover is a native `NSPopover`
([`macos/site_info_popover.rs`](../crates/photon-shell/src/platform/macos/site_info_popover.rs)),
which is Liquid Glass on macOS 26 and later. Other platforms draw the same
popover in GPUI
([`ui/omnibox/site_info.rs`](../crates/photon-shell/src/platform/ui/omnibox/site_info.rs)).
Both read one model,
[`platform/site_info.rs`](../crates/photon-shell/src/platform/site_info.rs),
for their controls, states and wording, and both change the site through the
omnibox's `toggle_site_control`, so a change to one belongs in the other.

### Not yet implemented

These show dimmed until the Engine can do them:

- **Certificates.** The connection page has a disabled "View certificate"
  button. "Secure" means only that the page's address is `https`
  (`notify_state` in `LibPhotonEmbedder`); RequestServer does not keep the
  peer certificate or TLS details. Showing a certificate needs RequestServer to
  record the main document's chain, the Engine to carry it through
  `LibPhotonEmbedder` and `photon-ffi` into page state, and the popover to
  show it: `SFCertificatePanel` on macOS and a GPUI view elsewhere. Security
  should then come from the TLS result rather than the scheme. Check how the
  Engine's Qt and AppKit UIs treat connection security first.
- **Camera** and **pop out view**, which have no Engine support.
- **Data and cookies** and **Permissions**, which have no pages to open.

## Who calls it

- `crates/photon-shell`'s omnibox
  ([`ui/omnibox/`](../crates/photon-shell/src/platform/ui/omnibox/mod.rs))
  resolves submitted text with the chosen search engine and asks its window
  to open the result, which loads a web page or one of the browser's own
  pages in the active tab. Startup addresses use the same rule.
- `photon-core` exposes the shared resolution rules and
  `normalize_url` with errors flattened for native callers.
- `crates/photon-ffi` implements `photon_browser_navigate` on top of core and
  owns the C string and output-buffer boundary.

The native shell owns the address entry and keeps typed text when a navigation
is rejected, so it can be corrected. There is no second UI-side copy of the
resolution rules.
