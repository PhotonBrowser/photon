# Photon development rules

1. Photon UI belongs in QML.
2. Browser/application logic and state belong in Rust.
3. C++ is for Qt native classes and engine glue only.
4. Web engine changes belong in `photon-engine`.
5. UI code must not reach into arbitrary engine internals.
6. Browser code should depend on an explicit Photon Engine embedder API.
7. Generated files stay out of source trees.
8. Keep Photon Engine regularly mergeable with Ladybird upstream.
9. Do not add another rendering or UI framework without an explicit architecture decision.
10. Keep ordinary UI work independent of engine internals.
11. Run linter and formatter commands: ./photon check (--verbose if needed) ./photon format#
12. Do not use computer use, unless you are ONLY screenshotting.
13. Always check Engine/UI/ source code for any features / implementations either the QT or the AppKit Ladybird browser has that can solve your problem if you ported it to QML.
14. Ensure everything is compatible with rust, tiny bit of c++, and QML.
