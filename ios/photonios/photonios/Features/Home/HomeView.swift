import SwiftUI
import UIKit

private enum SearchBarMotion {
    static let response: TimeInterval = 0.28
    static let focusDelay: TimeInterval = 0.22
    static let damping: CGFloat = 0.9
}

struct HomeView: View {
    @State private var touchLocation = CGPoint.zero
    @State private var isBackgroundPressing = false
    @State private var pressID = 0
    @State private var ripples: [DotRipple] = []
    @State private var isURLBarActive = false
    @State private var isKeyboardVisible = false
    @State private var urlText = ""
    @State private var searchActivationID = 0
    @FocusState private var isURLFieldFocused: Bool
    @Namespace private var searchBarNamespace

    var body: some View {
        ZStack {
            Color(uiColor: .systemBackground)
                .ignoresSafeArea()

            DotGridBackground(
                touchLocation: touchLocation,
                ripples: ripples,
                isTouching: isBackgroundPressing,
                onTouchChanged: updateBackgroundTouch,
                onTouchEnded: finishBackgroundTouch
            )

            VStack {
                Spacer()

                Group {
                    if isURLBarActive {
                        URLBarInput(
                            text: $urlText,
                            isFocused: $isURLFieldFocused,
                            namespace: searchBarNamespace,
                            activationID: searchActivationID,
                            onDismiss: closeURLBar,
                            onSubmit: submitSearch
                        )
                    } else {
                        SearchButton(namespace: searchBarNamespace, action: openURLBar)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.horizontal, isURLBarActive ? 16 : 0)
                .padding(.bottom, 8)
            }
            .frame(maxWidth: .infinity)
        }

        .sensoryFeedback(.impact(weight: .light, intensity: 0.35), trigger: pressID)
        .sensoryFeedback(.impact(weight: .light, intensity: 0.35), trigger: searchActivationID)
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
            isKeyboardVisible = true
        }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidHideNotification)) { _ in
            isKeyboardVisible = false
            if isURLBarActive {
                finishClosingURLBar()
            }
        }
    }

    private func updateBackgroundTouch(_ location: CGPoint) {
        if !isBackgroundPressing {
            isBackgroundPressing = true
            pressID += 1
        }

        touchLocation = location
    }

    private func finishBackgroundTouch(_ location: CGPoint) {
        touchLocation = location
        isBackgroundPressing = false
        if isURLBarActive {
            closeURLBar()
        }
        addRipple(at: location)
    }

    private func openURLBar() {
        withAnimation(searchBarAnimation) {
            isURLBarActive = true
        }
        searchActivationID += 1

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(SearchBarMotion.focusDelay))
            guard isURLBarActive else { return }
            isURLFieldFocused = true
        }
    }

    private func closeURLBar() {
        let keyboardWasVisible = isKeyboardVisible || isURLFieldFocused
        isURLFieldFocused = false
        if !keyboardWasVisible {
            finishClosingURLBar()
        }
    }

    private func finishClosingURLBar() {
        isURLFieldFocused = false
        withAnimation(searchBarAnimation) {
            isURLBarActive = false
        }
    }

    private func submitSearch() {
        closeURLBar()
    }

    private func addRipple(at origin: CGPoint) {
        let ripple = DotRipple(origin: origin, startedAt: .now)
        ripples.append(ripple)

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(DotGridMetrics.rippleDuration))
            ripples.removeAll { $0.id == ripple.id }
        }
    }

    private var searchBarAnimation: Animation {
        .spring(response: SearchBarMotion.response, dampingFraction: SearchBarMotion.damping)
    }
}

#Preview {
    HomeView()
}
