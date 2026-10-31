import Foundation
import WebKit

@MainActor
final class WebKitPage: NSObject, BrowserPage, WKNavigationDelegate {
    let webView: WKWebView
    private let eventHandler: BrowserPageEventHandler
    private var observations: [NSKeyValueObservation] = []
    private var hasPresentedFrame = false
    private var isStopping = false

    var platformView: AnyObject { webView }

    init(eventHandler: @escaping BrowserPageEventHandler) {
        self.eventHandler = eventHandler
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .default()
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        webView.navigationDelegate = self
        observePageState()
    }

    func load(_ url: URL) {
        webView.load(URLRequest(url: url))
    }

    func goBack() {
        webView.goBack()
    }

    func goForward() {
        webView.goForward()
    }

    func reload() {
        webView.reload()
    }

    func stopLoading() {
        isStopping = true
        webView.stopLoading()
        eventHandler(.navigationCancelled)
    }

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        isStopping = false
        eventHandler(.navigationStarted(reuseDisplayedFrame: hasPresentedFrame))
        reportState()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        hasPresentedFrame = true
        reportState()
        eventHandler(.framePresented)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        handle(error)
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        handle(error)
    }

    private func observePageState() {
        let reportLater: () -> Void = { [weak self] in
            Task { @MainActor [weak self] in self?.reportState() }
        }
        observations = [
            webView.observe(\.url, options: [.initial, .new]) { _, _ in reportLater() },
            webView.observe(\.title, options: [.initial, .new]) { _, _ in reportLater() },
            webView.observe(\.isLoading, options: [.initial, .new]) { _, _ in reportLater() },
            webView.observe(\.canGoBack, options: [.initial, .new]) { _, _ in reportLater() },
            webView.observe(\.canGoForward, options: [.initial, .new]) { _, _ in reportLater() },
        ]
    }

    private func reportState() {
        guard !isStopping else { return }
        eventHandler(
            .stateChanged(
                url: webView.url?.absoluteString ?? "",
                title: webView.title ?? "",
                isLoading: webView.isLoading,
                canGoBack: webView.canGoBack,
                canGoForward: webView.canGoForward
            )
        )
    }

    private func handle(_ error: Error) {
        let nsError = error as NSError
        if isStopping || (nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorCancelled) {
            isStopping = false
            eventHandler(.navigationCancelled)
            return
        }
        reportState()
        eventHandler(.loadFailed(error.localizedDescription))
    }
}

@MainActor
final class WebKitPageFactory: BrowserPageFactory {
    func makePage(eventHandler: @escaping BrowserPageEventHandler) -> BrowserPage {
        WebKitPage(eventHandler: eventHandler)
    }
}
