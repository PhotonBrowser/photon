# Omnibox

The address field in the titlebar is an omnibox: it takes text and decides what
it means. `crates/photon-omnibox` owns that decision, and nothing else in Photon
re-implements it.

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
[`suggest.rs`](../crates/photon-omnibox/src/suggest.rs) decides them from the
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

## Command bar

⌘T opens the command bar, as in Arc and Zen: a large field centred over the
page on frosted glass, rather than an empty new tab. In the vertical layout,
the sidebar's "+ New Tab" opens it too; the horizontal strip's + button and
the menu's New Tab still open a new tab page.

| With | It offers |
| --- | --- |
| Nothing typed | The open tabs, to switch to |
| Typed text | What Enter would open in the omnibox first, then matching open tabs (up to 4), matching commands (up to 3), then the omnibox's other suggestions |

Choosing an address or search opens it in a new tab, a `photon://` address
opens that page, an open tab is switched to, and a command runs. The commands
are New Tab, New Window, Reopen Closed Tab, Close Tab, Reload Page, Find in
Page, Copy Address, Show or Hide Sidebar, New Space, Next Space, Previous
Space and Settings, each offered only where it applies (page commands with a
web page showing, the sidebar and spaces in the vertical layout). They match by name or by keywords such as "refresh" or
"preferences". Typed words match where words start, as in the omnibox. The
arrow keys move through the rows, pointing selects one, Enter or a click
chooses it, and Escape or a click outside closes the bar.

The rules are in [`photon-core/src/command_bar.rs`](../crates/photon-core/src/command_bar.rs)
(with `match_words` from `photon-omnibox`), the view in
[`ui/command_bar/`](../crates/photon-shell/src/platform/ui/command_bar/mod.rs),
and carrying out the choice in
[`ui/window/command_bar.rs`](../crates/photon-shell/src/platform/ui/window/command_bar.rs).

## Who calls it

- `crates/photon-shell`'s omnibox
  ([`ui/omnibox/`](../crates/photon-shell/src/platform/ui/omnibox/mod.rs))
  resolves submitted text with the chosen search engine and asks its window
  to open the result, which loads a web page or one of the browser's own
  pages in the active tab. Startup addresses use the same rule.
- `crates/photon-core` re-exports the shared resolution rules and exposes
  `normalize_url` with errors flattened for native callers.
- `crates/photon-ffi` implements `photon_browser_navigate` on top of core and
  owns the C string and output-buffer boundary.

The native shell owns the address entry and keeps typed text when a navigation
is rejected, so it can be corrected. There is no second UI-side copy of the
resolution rules.
