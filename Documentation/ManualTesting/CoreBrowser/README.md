# Manual core browser comparison

Open `index.html` in Photon and in one current reference browser. Use the same viewport size, zoom, network, and page sequence in each. Record pass, fail, or unavailable for every case, along with a short note. A failed external service or permission policy should be marked separately from a rendering/engine failure.

## Setup

1. Record OS, Photon build/commit, reference browser name/version, window size, and network state.
2. Open `index.html` in both browsers. These fixtures are local and require no server.
3. If file URLs prevent storage or permission APIs, serve this directory via localhost and repeat the affected case.
4. Keep one clean browser profile for persistence checks, then repeat selected cases with a temporary/private profile.

## Cases

| Area | Actions | Pass criteria |
| --- | --- | --- |
| Startup and address entry | Launch Photon; enter explicit HTTPS URL, bare domain, `localhost:PORT`, search text, and an invalid/empty address. | Correct destination or clear error; address field remains editable; page title and icon update. |
| Tab lifecycle | New tab, open target-blank link, switch among tabs while pages load, close and reopen a tab, close last tab. | Correct active tab, expected page state retained, no crash or blank/stale content. |
| Navigation | On Navigation test, follow links, use back/forward, reload, stop a navigation, visit anchors, use push/replaceState then Back/Forward. | URL and displayed document stay in sync; toolbar enablement reflects history; no stuck loading. |
| Input | On Input and Forms test, type, select/copy/paste, use keyboard navigation, submit invalid and valid forms, invoke Find in Page and zoom. | Text input and selection work; validation identifies invalid fields; find count/highlight and zoom behave consistently. |
| Rendering | On Rendering test, resize, scroll page and nested scroller, click/tap, inspect text, SVG, canvas and broken image alt text. | Layout remains legible, controls respond, content paints without missing or stale regions. |
| Scripting and network | On Scripts and Network, start timer, fetch, open popup, download file, follow HTTPS link. | JavaScript executes; requests show expected result/error; popup policy and download UX are understandable. |
| Storage | On Storage test set each value, reload, open a second tab, close/reopen tab, restart browser; then clear site data in settings and retest. | localStorage/IndexedDB/cookies persist as applicable; sessionStorage is per-tab/session; clearing data removes it. |
| History and suggestions | Visit several fixture pages, search for a phrase, inspect address suggestions, remove one, clear history. | Visits/searches appear, suggestions match, removals and clear operations take effect. |
| Privacy and permissions | Test popup blocking, notification prompt on localhost/HTTPS, site-data clearing, temporary profile. | Prompts/policies are consistent, denied permissions remain denied, temporary profile does not retain state. |
| Stability | Repeat create/navigate/close at least 10 times; switch tabs during loading; leave animated/long content open; restart and restore session. | No crash, stuck input, unresponsive window, corrupted session, or obvious leak trend. |
| Compatibility/security | Compare a small set of known HTTPS sites, certificate-error fixture in a controlled environment, and a mobile/responsive page. | Secure indicators/errors are visible; no credentials entered on untrusted fixtures; layout and core input are comparable. |

## Record results

Use this compact format:

```text
Date / Photon commit / OS:
Reference browser and version:
Viewport and zoom:

Case | Photon (pass/fail/unavailable) | Reference | Notes
Startup/address |
Tab lifecycle |
Navigation |
Input |
Rendering |
Scripting/network |
Storage |
History/suggestions |
Privacy/permissions |
Stability |
Compatibility/security |
```

A reference browser gives a comparison point, not a guarantee that every browser should behave identically. Network services, OS-native controls, and permission policies can differ; focus on the same user-visible behavior and document those differences.
