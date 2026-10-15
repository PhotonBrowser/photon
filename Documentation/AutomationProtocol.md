# Photon Automation Protocol v1

Photon has an optional, local automation interface for isolated testing and
Velocity resource workloads. It is not enabled in normal browser sessions and
is not a remote debugging service.

## Enable an isolated session

Launch the Photon executable with:

```sh
Photon --automation-socket /private/session/Automation/photon.sock
```

The launch must also set `PHOTON_PROFILE_DIR` and `HOME` to private sibling
directories inside a private, current-user-owned session root. The socket's parent directory must already exist, be owned
by the current user, and have mode `0700`. Photon reads a sibling file named
`token`; it must be a current-user-owned regular file with mode `0600` and
contain a 256-bit hexadecimal token. The token must not be passed in argv.
Velocity creates both the isolated profile and credential for each managed
session. Photon keeps settings, browsing data, and its Engine profile under the
provided profile root.

Photon refuses to replace an existing socket. The socket is restricted to the
current user, every request requires token authentication, request and response
frames are limited to 64 KiB, and only one request is dispatched at a time.
Shutdown removes only the socket inode created by that Photon process. The
interface accepts only `about:blank` and absolute HTTP(S) URLs and provides no
shell, arbitrary JavaScript, preference, or filesystem commands.

Starting a managed executable directly is required. If macOS or another
launcher hands the request to an existing Photon process, Velocity's managed
process ownership check rejects the exited child and does not signal the
existing session.

## Transport and message format

The transport is a Unix domain stream socket using newline-delimited JSON.
Each connection carries one request and one response. Every request includes
`protocol_version`, a unique `request_id`, and the per-session
`authentication` token. The command and its optional `params` are flat at the
top level:

```json
{
  "protocol_version": 1,
  "request_id": "17",
  "authentication": "<session token>",
  "command": "navigate",
  "params": { "tab_id": 12, "url": "http://127.0.0.1:49152/simple?run=x&tab=0" }
}
```

A successful response correlates to the request ID:

```json
{
  "protocol_version": 1,
  "request_id": "17",
  "ok": true,
  "result": { "tab_id": 12, "navigation_id": "17", "state": "requested" }
}
```

Errors use a stable code and explanatory message:

```json
{
  "protocol_version": 1,
  "request_id": "18",
  "ok": false,
  "error": { "code": "tab_not_found", "message": "tab 99 was not found" }
}
```

## Commands

| Command | Parameters | Result |
| --- | --- | --- |
| `ping` | — | Server liveness |
| `get_version` | — | Application/protocol versions and supported commands |
| `list_windows` | — | Stable window IDs and current tab counts |
| `list_processes` | — | Read-only active process snapshot from `WebView::ProcessManager` and the macOS `ProcessReaper` |
| `list_tabs` | optional `window_id` | Tab/window IDs, URL, active/loading/error state, engine load-finished state, and document generation |
| `new_tab` | `window_id`, `url` | The ID of a real Photon tab |
| `close_tab` | `tab_id` | Accepted closing state, using ordinary tab lifecycle logic |
| `activate_tab` | `tab_id` | Activates an existing tab |
| `navigate` | `tab_id`, `url` | Navigation request ID and requested state |
| `reload` | `tab_id` | Reloads the current page through Photon's navigation command |
| `get_tab_status` | `tab_id` | Current URL, loading/error state, crash status, engine load-finished state, document generation, and window/tab counts |
| `probe_speedometer_result` | `tab_id`, `document_generation` | Installs the restricted read-only observer for the expected document generation |

`list_processes` returns active records in the actual
`WebView::ProcessManager` registry, plus macOS's separate `ProcessReaper`
lifecycle helper when it was started successfully. Roles come from Ladybird's
`ProcessType` values; `process_reaper` identifies the auxiliary lifecycle
helper. Each record includes its PID, running state, and process and parent
start identities captured when its process handle was registered. The process
start time also serves as a launch-generation identity, so a recycled PID is
distinguishable. The `complete` flag applies only to this explicitly named
scope; it does not claim that every unrelated helper or native service belongs
to that scope. If Photon could not capture a process or parent identity, the
corresponding field is null and the snapshot's `complete` flag is false.

Example response (identifiers and timestamps are illustrative):

```json
{
  "protocol_version": 1,
  "request_id": "19",
  "ok": true,
  "result": {
    "scope": "active WebView::ProcessManager processes and its ProcessReaper on macOS",
    "lifecycle_source": "WebView::ProcessManager and ProcessReaper",
    "complete": true,
    "processes": [
      {
        "pid": 1234,
        "role": "application",
        "state": "running",
        "start_time": { "seconds": 1791926400, "microseconds": 123456 },
        "launch_generation": "1791926400123456",
        "parent_identity": {
          "pid": 1200,
          "start_time": { "seconds": 1791926300, "microseconds": 654321 }
        }
      }
    ]
  }
}
```

The server reads the process snapshot from the application-owned manager on
the GPUI thread, then serializes and returns it over the existing bounded
socket request path. It does not signal or otherwise control reported
processes. Velocity independently verifies live PIDs, process and parent start
times, user, private process session, and the root executable before collecting
resource data. It allows a helper to have been reparented only when the
originally registered parent identity still verifies.

Window and tab IDs are opaque-to-callers unsigned integer identifiers that
remain stable for the lifetime of the corresponding UI object. They are not
GPUI entity pointers. All application state changes are dispatched onto the
GPUI event loop; the socket worker only validates frames and transfers bounded
requests. Navigation acknowledgement means the existing navigation command
was accepted, not that loading finished. `document_ready` remains `null`;
`engine_load_finished` separately reports the WebView's `on_load_finish`
callback and is not a substitute for application-specific readiness. Velocity
also verifies its local fixtures with a run-and-tab-specific HTTP beacon after
the document `load` event. The `document_generation` identifies the loaded
document for observer requests and lets the caller reject state from a replaced
navigation.

## Velocity integration

Velocity generates one token per iteration using the operating system random
source, creates mode-`0700` profile and short-path socket directories plus a
mode-`0600` token file, then launches Photon with the socket path. The socket
directory is separate from the browser profile to stay below macOS's Unix
socket path limit. Velocity removes it only after managed process shutdown is
verified and its token still matches the session. It negotiates protocol v1
before treating Photon tab capabilities as available. The isolated default
page is controlled as a real tab; page workloads navigate that tab and
multi-tab workloads create the requested number of separate tabs. Velocity
verifies the observed tab IDs/counts and waits for an individual readiness
beacon for every requested fixture page before starting resource sampling.
The same resource collector and reporting pipeline is used for Photon and
other supported browsers.

Photon's normal launch path remains unchanged when the automation argument is
absent. Developers can use this interface for isolated tests by creating a
private profile and socket directory, writing a fresh token file, and
launching a separate Photon executable with the explicit socket argument.

## Restricted Speedometer result observer

Automation protocol v1 additionally advertises `probe_speedometer_result` in
Photon builds that implement it. The read-only command takes
`{ "tab_id": N, "document_generation": G }` and installs a fixed observer in
that tab only if the current document generation still matches. It accepts
only the official
`https://browserbench.org/Speedometer3.1/` page or a `127.0.0.1` URL under
`/Speedometer3.1/`. It does not accept JavaScript from the caller and cannot
change benchmark suites, iteration counts, preferences, or browser state beyond
observing the official page result.

The observer watches Speedometer's official running/summary document state and
publishes a structured JSON payload in that isolated tab's document title:

```json
{"state":"running","version":"3.1","current_suite":"TodoMVC-JavaScript-ES5"}
```

or, after the official benchmark finishes:

```json
{"state":"complete","version":"3.1","score":12.5,"internal_scores":[12.3,12.7]}
```

The values above are illustrative. Invalid official summaries use
`{"state":"failed","version":"3.1","error":"..."}`. Velocity reads the
title through `list_tabs`/`get_tab_status`, validates the version and finite
score, and never treats the observer command acknowledgement as completion.
While running, the observer reads Speedometer's own progress attributes and
performance marks to identify the current suite and test. It also reports the
document generation and visibility, the current test-frame URL/readiness/
visibility, bounded parent/frame exceptions, and time since official progress.
These fields are read-only diagnostics and do not change the benchmark workload
or score.
The observer is installed only after Photon has received the document's
load-finished callback. `get_tab_status` exposes this as
`engine_load_finished`, sourced from the embedder's actual `on_load_finish`
callback. Velocity waits for that signal and checks `error_state` before asking
to install the observer. If the engine has no active document page, the command
returns a `page_unavailable` error instead of invoking the JavaScript path.
Viewport warnings are included in observer payloads when the page reports
Speedometer's minimum viewport warning.
For authenticated automation runs, setting
`PHOTON_AUTOMATION_LIFECYCLE_TRACE=1` emits at most 64 app/window lifecycle
events to standard error. Normal Photon sessions do not enable this trace.
The observer is available only in an authenticated, explicitly enabled,
isolated automation session. No credential or score is sent to Photon logs.
