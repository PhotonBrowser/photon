# Velocity fixtures

All pages are served by Velocity from loopback and have no remote assets.

- `simple`: small static document. Velocity injects only the common readiness
  beacon; the fixture itself has no workload JavaScript.
- `complex`: deterministic 128-card DOM with grid layout, borders, shadows,
  and a responsive breakpoint.
- `dynamic`: deterministic DOM updates, configured by `intensity` and
  `update_ms` query values.

Fixture version is recorded in every resource benchmark report. Increment it
when page structure or workload behavior changes.
