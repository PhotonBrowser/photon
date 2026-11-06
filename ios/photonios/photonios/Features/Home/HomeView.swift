import SwiftUI

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
            didHide: keyboardDidHide
        )
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
        if isURLBarActive {
            OmniboxPanel(
                text: $urlText,
                isFocused: $isURLFieldFocused,
                showsBorderBeam: isURLBarSettled,
                onSubmit: closeURLBar
            )
            .transition(panelTransition)
        }
    }

    private var panelTransition: AnyTransition {
        guard !reduceMotion else { return .opacity }

        return .asymmetric(
            insertion: .opacity.combined(
                with: .scale(scale: 0.98, anchor: .bottom)
            ),
            removal: .move(edge: .bottom)
        )
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
        isURLBarSettled = isURLBarActive
    }

    private func keyboardDidHide() {
        let animation = reduceMotion
            ? Animation.easeOut(duration: HomeMotion.reducedMotionDuration)
            : .easeInOut(duration: HomeMotion.keyboardDismissDuration)

        withAnimation(animation) {
            isKeyboardVisible = false
            isURLBarSettled = false
            isURLFieldFocused = false
            isURLBarActive = false
        }
    }
}

#Preview {
    HomeView()
}
