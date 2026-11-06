import SwiftUI
import UIKit

private enum HomeMotion {
    static let panelResponse: TimeInterval = 0.28
    static let panelDamping: CGFloat = 1.0
    static let keyboardDismissDuration: TimeInterval = 0.2
    static let reducedMotionDuration: TimeInterval = 0.16
}

struct HomeView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isURLBarActive = false
    @State private var isURLBarSettled = false
    @State private var isKeyboardVisible = false
    @State private var isKeyboardHiding = false
    @State private var panelHeight = URLBarStyle.panelHeight
    @State private var urlText = ""
    @FocusState private var isURLFieldFocused: Bool

    var body: some View {
        ZStack {
            Color(uiColor: .systemBackground)
                .ignoresSafeArea()

            homeScrollView
        }
        .onKeyboardLifecycle(
            willShow: keyboardWillShow,
            didShow: keyboardDidShow,
            willHide: keyboardWillHide,
            didHide: keyboardDidHide
        )
        .onPreferenceChange(OmniboxPanelHeightPreference.self) { height in
            guard height > 0, abs(panelHeight - height) > 0.5 else { return }
            panelHeight = height
        }
    }

    private var homeScrollView: some View {
        ScrollView(.vertical) {
            ZStack(alignment: .bottom) {
                DotGridBackground(
                    isURLBarActive: isURLBarActive,
                    onBackgroundTap: closeURLBar
                )

                VStack(spacing: 0) {
                    Spacer(minLength: 0)
                    searchButton
                }
                .frame(maxWidth: .infinity)
            }
            .containerRelativeFrame(.vertical)
        }
        .scrollIndicators(.hidden)
        .scrollDismissesKeyboard(.interactively)
        .scrollBounceBehavior(isURLBarActive ? .always : .basedOnSize, axes: .vertical)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            omniboxPanel
        }
    }

    @ViewBuilder
    private var searchButton: some View {
        if !isURLBarActive && !isKeyboardVisible {
            SearchButton(action: openURLBar)
                .padding(.bottom, 8)
                .transition(.opacity)
        }
    }

    @ViewBuilder
    private var omniboxPanel: some View {
        if isURLBarActive || isKeyboardHiding {
            OmniboxPanel(
                text: $urlText,
                isFocused: $isURLFieldFocused,
                showsBorderBeam: isURLBarSettled,
                onSubmit: closeURLBar
            )
            .offset(y: isKeyboardHiding && !reduceMotion ? panelHeight : 0)
            .transition(panelTransition)
        }
    }

    private var panelTransition: AnyTransition {
        let insertion: AnyTransition = reduceMotion
            ? .opacity
            : .opacity.combined(with: .scale(scale: 0.98, anchor: .bottom))
        let removal: AnyTransition

        if reduceMotion {
            removal = .opacity
        } else if isKeyboardHiding {
            removal = .identity
        } else {
            removal = .move(edge: .bottom)
        }

        return .asymmetric(insertion: insertion, removal: removal)
    }

    private var panelAnimation: Animation {
        reduceMotion
            ? .easeOut(duration: HomeMotion.reducedMotionDuration)
            : .spring(response: HomeMotion.panelResponse, dampingFraction: HomeMotion.panelDamping)
    }

    private func openURLBar() {
        withAnimation(panelAnimation) {
            isURLBarSettled = false
            isURLBarActive = true
        }
    }

    private func closeURLBar() {
        guard isKeyboardVisible else {
            finishClosingURLBar()
            return
        }

        isURLFieldFocused = false
    }

    private func finishClosingURLBar() {
        isURLFieldFocused = false
        isURLBarSettled = false
        withAnimation(panelAnimation) {
            isURLBarActive = false
        }
    }

    private func keyboardWillShow() {
        isKeyboardVisible = true
    }

    private func keyboardDidShow() {
        isKeyboardVisible = true
        isKeyboardHiding = false
        isURLBarSettled = isURLBarActive
    }

    private func keyboardWillHide(_ notification: Notification) {
        guard isURLBarActive else { return }

        withAnimation(keyboardDismissAnimation(for: notification)) {
            isURLBarSettled = false
            isURLFieldFocused = false
            isKeyboardHiding = true
        }
    }

    private func keyboardDidHide() {
        if reduceMotion {
            withAnimation(.easeOut(duration: HomeMotion.reducedMotionDuration)) {
                resetKeyboardState()
            }
            return
        }

        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction) {
            resetKeyboardState()
        }
    }

    private func resetKeyboardState() {
        isKeyboardVisible = false
        isURLBarSettled = false
        isURLFieldFocused = false
        isURLBarActive = false
        isKeyboardHiding = false
    }

    private func keyboardDismissAnimation(for notification: Notification) -> Animation {
        let userInfo = notification.userInfo ?? [:]
        let duration = userInfo[UIResponder.keyboardAnimationDurationUserInfoKey] as? TimeInterval
            ?? HomeMotion.keyboardDismissDuration

        guard !reduceMotion else {
            return .easeOut(duration: min(duration, HomeMotion.reducedMotionDuration))
        }

        let curve = (userInfo[UIResponder.keyboardAnimationCurveUserInfoKey] as? NSNumber)?.intValue
        if curve == UIView.AnimationCurve.easeIn.rawValue {
            return .timingCurve(0.42, 0, 1, 1, duration: duration)
        }
        if curve == UIView.AnimationCurve.easeOut.rawValue {
            return .timingCurve(0, 0, 0.58, 1, duration: duration)
        }
        if curve == UIView.AnimationCurve.linear.rawValue {
            return .linear(duration: duration)
        }

        return .timingCurve(0.42, 0, 0.58, 1, duration: duration)
    }
}

#Preview {
    HomeView()
}
