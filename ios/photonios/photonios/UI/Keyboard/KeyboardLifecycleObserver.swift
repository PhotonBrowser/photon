import SwiftUI
import UIKit

struct KeyboardLifecycleObserver: ViewModifier {
    let onWillShow: () -> Void
    let onDidShow: () -> Void
    let onDidHide: () -> Void

    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
                onWillShow()
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidShowNotification)) { _ in
                onDidShow()
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidHideNotification)) { _ in
                onDidHide()
            }
    }
}

extension View {
    func onKeyboardLifecycle(
        willShow: @escaping () -> Void,
        didShow: @escaping () -> Void,
        didHide: @escaping () -> Void
    ) -> some View {
        modifier(
            KeyboardLifecycleObserver(
                onWillShow: willShow,
                onDidShow: didShow,
                onDidHide: didHide
            )
        )
    }
}
