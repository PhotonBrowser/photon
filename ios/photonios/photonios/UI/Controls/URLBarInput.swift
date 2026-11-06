import BorderBeamKit
import SwiftUI

enum URLBarStyle {
    static let fieldHeight: CGFloat = 48
    static let fieldRadius: CGFloat = fieldHeight / 2
    static let cardRadius: CGFloat = fieldRadius + 16
    static let horizontalInset: CGFloat = 16
    static let verticalInset: CGFloat = 12
    static let historySectionHeight: CGFloat = 152
    static let historyHorizontalInset: CGFloat = 24
    static let historyVerticalInset: CGFloat = 12
    static let historyRowSpacing: CGFloat = 12
    static let borderBeamCycleDuration: TimeInterval = 3.1
}

struct URLBarInput: View {
    @Binding var text: String
    @FocusState.Binding var isFocused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isBorderBeamAnimating = false
    @State private var borderBeamPlaybackID = 0
    @State private var hasPlayedBorderBeam = false

    let showsBorderBeam: Bool
    let onSubmit: () -> Void

    var body: some View {
        BorderBeam(
            size: .line,
            colorVariant: .colorful,
            theme: .auto,
            duration: URLBarStyle.borderBeamCycleDuration,
            active: isBorderBeamAnimating,
            borderRadius: Double(URLBarStyle.fieldRadius)
        ) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)

                TextField("Search or enter URL", text: $text)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .submitLabel(.go)
                    .focused($isFocused)
                    .onAppear { isFocused = true }
                    .onSubmit(onSubmit)

                if !text.isEmpty {
                    Button {
                        text = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(.tertiary)
                    }
                    .accessibilityLabel("Clear address")
                }
            }
            .padding(.horizontal, URLBarStyle.horizontalInset)
            .frame(maxWidth: .infinity, minHeight: URLBarStyle.fieldHeight)
            .glassEffect(.regular, in: Capsule())
        }
        .padding(.horizontal, URLBarStyle.horizontalInset)
        .padding(.top, URLBarStyle.verticalInset)
        .padding(.bottom, URLBarStyle.verticalInset)
        .frame(maxWidth: .infinity)
        .onAppear {
            if showsBorderBeam {
                startBorderBeamPlayback()
            }
        }
        .onChange(of: showsBorderBeam) { _, shouldPlay in
            if shouldPlay {
                startBorderBeamPlayback()
            } else {
                stopBorderBeamPlayback()
            }
        }
        .onChange(of: reduceMotion) { _, shouldReduceMotion in
            if shouldReduceMotion {
                stopBorderBeamPlayback()
            } else if showsBorderBeam {
                startBorderBeamPlayback()
            }
        }
        .task(id: borderBeamPlaybackID) {
            guard isBorderBeamAnimating else { return }

            do {
                try await Task.sleep(for: .seconds(URLBarStyle.borderBeamCycleDuration))
            } catch {
                return
            }

            guard !Task.isCancelled else { return }
            isBorderBeamAnimating = false
        }
    }

    private func startBorderBeamPlayback() {
        guard !reduceMotion, !hasPlayedBorderBeam else { return }
        hasPlayedBorderBeam = true
        borderBeamPlaybackID += 1
        isBorderBeamAnimating = true
    }

    private func stopBorderBeamPlayback() {
        guard isBorderBeamAnimating else { return }
        borderBeamPlaybackID += 1
        isBorderBeamAnimating = false
    }
}
