import SwiftUI

private enum URLBarSweepStyle {
    static let duration: TimeInterval = 0.7
    static let bandWidth: CGFloat = 96
    static let bandHeight: CGFloat = 3
    static let hues = (0...7).map {
        Color(hue: Double($0) / 7, saturation: 0.95, brightness: 1)
    }
    static let gradient = LinearGradient(
        colors: hues,
        startPoint: .leading,
        endPoint: .trailing
    )
    static let edgeFade = LinearGradient(
        stops: [
            .init(color: .clear, location: 0),
            .init(color: .white, location: 0.2),
            .init(color: .white, location: 0.8),
            .init(color: .clear, location: 1),
        ],
        startPoint: .leading,
        endPoint: .trailing
    )
}

struct URLBarInput: View {
    @Binding var text: String
    @FocusState.Binding var isFocused: Bool
    @State private var sweepProgress: CGFloat = 0
    @State private var isSweepVisible = true

    let namespace: Namespace.ID
    let activationID: Int
    let onDismiss: () -> Void
    let onSubmit: () -> Void

    var body: some View {
        ZStack {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                    .transition(.blurReplace)

                TextField("Search or enter URL", text: $text)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .submitLabel(.go)
                    .focused($isFocused)
                    .onSubmit(onSubmit)
                    .transition(.blurReplace)
            }
            .padding(.horizontal, 16)
            .frame(maxWidth: .infinity, minHeight: 48)
            .glassEffect(.regular, in: Capsule())

            GeometryReader { geometry in
                Rectangle()
                    .fill(URLBarSweepStyle.gradient)
                    .frame(
                        width: URLBarSweepStyle.bandWidth,
                        height: URLBarSweepStyle.bandHeight
                    )
                    .mask(URLBarSweepStyle.edgeFade)
                    .position(
                        x: -URLBarSweepStyle.bandWidth / 2
                            + (geometry.size.width + URLBarSweepStyle.bandWidth)
                                * sweepProgress,
                        y: geometry.size.height - URLBarSweepStyle.bandHeight / 2
                    )
            }
            .clipShape(Capsule())
            .opacity(isSweepVisible ? 1 : 0)
            .allowsHitTesting(false)
        }
        .matchedGeometryEffect(id: "search-url-bar", in: namespace)
        .task(id: activationID) {
            sweepProgress = 0
            isSweepVisible = true
            await Task.yield()
            withAnimation(
                .linear(duration: URLBarSweepStyle.duration),
                completionCriteria: .logicallyComplete
            ) {
                sweepProgress = 1
            } completion: {
                isSweepVisible = false
            }
        }
        .simultaneousGesture(
            DragGesture(minimumDistance: 12)
                .onEnded { value in
                    guard value.translation.height > 40,
                          abs(value.translation.height) > abs(value.translation.width)
                    else { return }
                    onDismiss()
                }
        )
    }
}
