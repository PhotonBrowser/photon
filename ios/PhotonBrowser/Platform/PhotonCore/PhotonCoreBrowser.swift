import Foundation

enum BrowserCoreError: LocalizedError {
    case addressResolution(String)

    var errorDescription: String? {
        switch self {
        case let .addressResolution(message): message
        }
    }
}

/// Owns one `photon-ffi` browser handle. Keep the handle on the main actor because
/// the WebKit adapter reports its state from the main thread.
@MainActor
final class PhotonCoreBrowser {
    private let handle: OpaquePointer

    init() {
        guard let handle = photon_browser_create() else {
            fatalError("photon_browser_create returned a null handle")
        }
        self.handle = handle
    }

    deinit {
        photon_browser_destroy(handle)
    }

    var state: BrowserPageState {
        let error = string(PHOTON_BROWSER_STRING_ERROR)
        return BrowserPageState(
            url: string(PHOTON_BROWSER_STRING_URL),
            title: string(PHOTON_BROWSER_STRING_TITLE),
            isLoading: flag(PHOTON_BROWSER_FLAG_LOADING),
            canGoBack: flag(PHOTON_BROWSER_FLAG_CAN_GO_BACK),
            canGoForward: flag(PHOTON_BROWSER_FLAG_CAN_GO_FORWARD),
            error: error.isEmpty ? nil : error
        )
    }

    func resolveAddress(_ address: String) throws -> URL {
        let (encodedInputSize, overflow) = address.utf8.count.multipliedReportingOverflow(by: 3)
        guard !overflow else {
            throw BrowserCoreError.addressResolution("Address is too long")
        }
        let (capacity, additionOverflow) = encodedInputSize.addingReportingOverflow(256)
        guard !additionOverflow else {
            throw BrowserCoreError.addressResolution("Address is too long")
        }

        var buffer = [CChar](repeating: 0, count: max(capacity, 256))
        let succeeded = address.withCString { input in
            buffer.withUnsafeMutableBufferPointer { output in
                photon_browser_navigate(handle, input, output.baseAddress, output.count)
            }
        }

        guard succeeded, let terminator = buffer.firstIndex(of: 0) else {
            throw BrowserCoreError.addressResolution(
                optionalString(photon_browser_error(handle)) ?? "Unable to resolve address"
            )
        }

        let target = String(decoding: buffer[..<terminator].map(UInt8.init(bitPattern:)), as: UTF8.self)
        guard let url = URL(string: target) else {
            throw BrowserCoreError.addressResolution("Rust returned an invalid URL")
        }
        return url
    }

    func navigationStarted(reuseDisplayedFrame: Bool) {
        photon_browser_navigation_started(handle, reuseDisplayedFrame)
    }

    func update(url: String, title: String, isLoading: Bool, canGoBack: Bool, canGoForward: Bool) {
        url.withCString { urlPointer in
            title.withCString { titlePointer in
                photon_browser_update(
                    handle,
                    urlPointer,
                    titlePointer,
                    isLoading,
                    canGoBack,
                    canGoForward
                )
            }
        }
    }

    func framePresented() {
        photon_browser_frame_presented(handle)
    }

    func cancelNavigation() {
        photon_browser_cancel_navigation(handle)
    }

    func loadFailed(_ message: String) {
        message.withCString { photon_browser_load_failed(handle, $0) }
    }

    func allows(_ command: UInt32) -> Bool {
        photon_browser_command(handle, command)
    }

    private func string(_ field: PhotonBrowserStringField) -> String {
        optionalString(photon_browser_string(handle, field.rawValue)) ?? ""
    }

    private func flag(_ field: PhotonBrowserFlag) -> Bool {
        photon_browser_flag(handle, field.rawValue)
    }

    private func optionalString(_ pointer: UnsafePointer<CChar>?) -> String? {
        guard let pointer, pointer.pointee != 0 else { return nil }
        return String(cString: pointer)
    }
}
