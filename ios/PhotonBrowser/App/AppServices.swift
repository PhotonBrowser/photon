import Foundation

/// Composition root for the iOS browser services.
@MainActor
final class AppServices {
    let webBrowser: WebKitBrowserAdapter

    init() {
        webBrowser = WebKitBrowserAdapter(pageFactory: WebKitPageFactory())
    }
}
