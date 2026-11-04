import SwiftUI

struct HomeView: View {
    @State private var touchLocation = CGPoint.zero
    @State private var isBackgroundPressing = false
    @State private var pressID = 0
    @State private var ripples: [DotRipple] = []

    var body: some View {
        ZStack {
            Color(uiColor: .systemBackground)
                .ignoresSafeArea()

            DotGridBackground(
                touchLocation: touchLocation,
                ripples: ripples,
                isTouching: isBackgroundPressing
            )

            VStack {
                Spacer()
                SearchButton {}
            }
        }
        .contentShape(Rectangle())
        .simultaneousGesture(
            DragGesture(minimumDistance: 0, coordinateSpace: .global)
                .onChanged { value in
                    if !isBackgroundPressing {
                        isBackgroundPressing = true
                        pressID += 1
                    }

                    touchLocation = value.location
                }
                .onEnded { value in
                    touchLocation = value.location
                    isBackgroundPressing = false
                    addRipple(at: value.location)
                }
        )

        .sensoryFeedback(.impact(weight: .light, intensity: 0.35), trigger: pressID)
    }

    private func addRipple(at origin: CGPoint) {
        let ripple = DotRipple(origin: origin, startedAt: .now)
        ripples.append(ripple)

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(DotGridMetrics.rippleDuration))
            ripples.removeAll { $0.id == ripple.id }
        }
    }
}

#Preview {
    HomeView()
}
