import Foundation

/// Platform page surface and commands, without exposing a specific rendering framework.
@MainActor
protocol BrowserPage: AnyObject {
    var platformView: AnyObject { get }

    func load(_ url: URL)
    func goBack()
    func goForward()
    func reload()
    func stopLoading()
}

@MainActor
protocol BrowserPageFactory {
    func makePage(eventHandler: @escaping BrowserPageEventHandler) -> BrowserPage
}
