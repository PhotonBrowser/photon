import Foundation

struct BrowserPageState: Equatable {
    var url = ""
    var title = ""
    var isLoading = false
    var canGoBack = false
    var canGoForward = false
    var error: String?
}

enum BrowserPageEvent {
    case navigationStarted(reuseDisplayedFrame: Bool)
    case stateChanged(url: String, title: String, isLoading: Bool, canGoBack: Bool, canGoForward: Bool)
    case framePresented
    case navigationCancelled
    case loadFailed(String)
}

typealias BrowserPageEventHandler = @MainActor (BrowserPageEvent) -> Void
