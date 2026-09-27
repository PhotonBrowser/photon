# Photon

Photon is a Qt Quick browser application with application logic owned by Rust and web technology provided by Photon Engine.

```text
PhotonBrowser/photon (this repository)
  Rust application and CLI · QML presentation · native Qt integration
  Engine/ pins one exact commit
      ↓
PhotonBrowser/photon-engine
  Ladybird-derived engine and Photon embedding work
      ↓ upstream remote
LadybirdBrowser/ladybird
```

See [Building](Documentation/Building.md), [Architecture](Documentation/Architecture.md), and [Contributing](CONTRIBUTING.md). Photon Engine retains Ladybird attribution and licensing; it is an actively maintained downstream, not a source mirror.
