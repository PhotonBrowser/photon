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
with Google as the default. Another engine is one value:

```rust
let mut engines = SearchEngines::builtin();
engines.register(kagi)?;      // an id that already exists is replaced
engines.set_default("kagi");   // false when the id is unknown
resolve_with("photon browser", &engines);
```

Registration validates the template, so a malformed endpoint is rejected when it
is registered rather than producing an unopenable address when someone presses
Enter.

## Who calls it

- `crates/photon-app` calls `photon_omnibox::resolve` in
  `EngineSession::navigate`, the one place the shell asks the engine to open
  something. The `<photon-webview url="…">` prop therefore takes typed text, and
  the field and the engine cannot disagree about what Enter opened.
- `crates/photon-core` re-exports the crate, and `photon_browser_navigate` maps a
  resolution onto the engine navigation target. `normalize_url` is the same call
  with the error flattened to the strings the C ABI reports.

The native shell owns the address entry and keeps typed text when a navigation
is rejected, so it can be corrected. There is no second UI-side copy of the
resolution rules.
