import Foundation

/// Adapts Rust browser state and commands to one native WebKit page.
/// Browser policy and authoritative page state stay in photon-core.
@MainActor
final class WebKitBrowserAdapter {
    private let core: PhotonCoreBrowser
    private var pageStorage: BrowserPage!

    private(set) var state: BrowserPageState
    var onStateChange: ((BrowserPageState) -> Void)?

    var page: BrowserPage { pageStorage }

    init(pageFactory: BrowserPageFactory) {
        core = PhotonCoreBrowser()
        state = core.state
        pageStorage = pageFactory.makePage { [weak self] event in
            self?.receive(event)
        }
    }

    func navigate(_ address: String) throws {
        page.load(try core.resolveAddress(address))
    }

    func goBack() {
        guard core.allows(PHOTON_BROWSER_COMMAND_BACK.rawValue) else { return }
        page.goBack()
    }

    func goForward() {
        guard core.allows(PHOTON_BROWSER_COMMAND_FORWARD.rawValue) else { return }
        page.goForward()
    }

    func reload() {
        guard core.allows(PHOTON_BROWSER_COMMAND_RELOAD.rawValue) else { return }
        page.reload()
    }

    func stopLoading() {
        guard core.allows(PHOTON_BROWSER_COMMAND_STOP_LOADING.rawValue) else { return }
        page.stopLoading()
    }

    private func receive(_ event: BrowserPageEvent) {
        switch event {
        case let .navigationStarted(reuseDisplayedFrame):
            core.navigationStarted(reuseDisplayedFrame: reuseDisplayedFrame)
        case let .stateChanged(url, title, isLoading, canGoBack, canGoForward):
            core.update(
                url: url,
                title: title,
                isLoading: isLoading,
                canGoBack: canGoBack,
                canGoForward: canGoForward
            )
        case .framePresented:
            core.framePresented()
        case .navigationCancelled:
            core.cancelNavigation()
        case let .loadFailed(message):
            core.loadFailed(message)
        }
        state = core.state
        onStateChange?(state)
    }
}
