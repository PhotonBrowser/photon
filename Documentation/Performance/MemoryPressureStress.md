# Memory pressure stress scenario

Run an isolated, repeatable pressure scenario against the macOS Photon Engine
build. It opens local pages that allocate transient typed arrays and draw a
canvas, then injects warning, normal and critical pressure through the
authenticated automation socket.

Build Photon and initialize the development presentation service once:

```sh
./photon build
./photon run --shutdown-after-seconds 2
```

Run the scenario with the release executable:

```sh
python3 scripts/memory_pressure_stress.py --photon target/release/Photon
```

The runner creates a private temporary `HOME`, profile, token and Unix socket.
It removes that session after Photon has shut down. It binds one loopback HTTP
fixture per tab on a unique port, giving each page a distinct origin. It samples
process RSS directly and includes per-tab memory estimates when the Engine
provides them.

The scenario checks that warning pressure releases hidden memory while keeping
all pages loaded; normal pressure cancels warning cleanup; critical pressure
discards four eligible hidden tabs per pass while preserving the active tab;
returning to normal cancels the pending critical retry; and a second critical
period retries until all eligible hidden test tabs are discarded. It then
compares summed WebContent RSS before and after cleanup and parses the
Compositor's GPU cache purge measurements from the Photon log.

The run passes the process-memory check only when WebContent RSS falls by at
least 16 MiB. It requires a GPU cache metric in the log; if the reported cache
was nonempty, at least one pressure purge must reduce it. When the cache reports
zero bytes before every purge, the GPU check reports that there was nothing
available to reclaim. Pass `--allow-unobserved-gpu-reclaim` to continue when the
build or driver does not expose those log measurements. Increase workload with
`--tabs` or `--mib-per-tab`, and adjust the RSS floor with
`--minimum-webcontent-reclaimed-mib`.

The runner is intentionally opt-in. It exercises the real page discard path in
a disposable browser profile and may briefly use several hundred MiB of memory.
