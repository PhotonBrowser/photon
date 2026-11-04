//
//  ContentView.swift
//  photonios
//
//  Created by Theo Slater on 04/11/2026.
//

import SwiftUI

private enum DotGridMetrics {
    static let spacing: CGFloat = 24
    static let diameter: CGFloat = 2
    static let opacity = 0.14
    static let rippleWidth: CGFloat = 48
    static let rippleDuration = 0.8
}

private struct DotGridBackground: View {
    let rippleOrigin: CGPoint
    let rippleProgress: CGFloat
    let isRippleActive: Bool

    var body: some View {
        Canvas { context, size in
            var dots = Path()
            var rippleDots = Path()
            let rippleRadius = farthestCornerDistance(in: size) * rippleProgress

            for y in stride(from: 0, through: size.height, by: DotGridMetrics.spacing) {
                for x in stride(from: 0, through: size.width, by: DotGridMetrics.spacing) {
                    let rect = CGRect(
                        x: x - DotGridMetrics.diameter / 2,
                        y: y - DotGridMetrics.diameter / 2,
                        width: DotGridMetrics.diameter,
                        height: DotGridMetrics.diameter
                    )
                    dots.addEllipse(in: rect)

                    if isRippleActive {
                        let distance = hypot(x - rippleOrigin.x, y - rippleOrigin.y)
                        let intensity = max(
                            0,
                            1 - abs(distance - rippleRadius) / DotGridMetrics.rippleWidth
                        )

                        if intensity > 0.05 {
                            let diameter = DotGridMetrics.diameter * (1 + intensity * 1.4)
                            rippleDots.addEllipse(
                                in: CGRect(
                                    x: x - diameter / 2,
                                    y: y - diameter / 2,
                                    width: diameter,
                                    height: diameter
                                )
                            )
                        }
                    }
                }
            }

            context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.opacity)))
            context.fill(rippleDots, with: .color(Color.accentColor.opacity(0.42)))
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }

    private func farthestCornerDistance(in size: CGSize) -> CGFloat {
        [
            CGPoint.zero,
            CGPoint(x: size.width, y: 0),
            CGPoint(x: 0, y: size.height),
            CGPoint(x: size.width, y: size.height),
        ]
        .map { hypot($0.x - rippleOrigin.x, $0.y - rippleOrigin.y) }
        .max() ?? 0
    }
}

struct ContentView: View {
    @State private var rippleOrigin = CGPoint.zero
    @State private var rippleProgress: CGFloat = 0
    @State private var isRippleActive = false
    @State private var isBackgroundPressing = false
    @State private var rippleID = 0

    var body: some View {
        ZStack {
            Color(uiColor: .systemBackground)
                .ignoresSafeArea()

            DotGridBackground(
                rippleOrigin: rippleOrigin,
                rippleProgress: rippleProgress,
                isRippleActive: isRippleActive
            )

            VStack {
                Spacer()

                Button {} label: {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 20, weight: .regular))
                        .foregroundStyle(.primary)
                        .frame(width: 100, height: 34)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.glass)
                .accessibilityLabel("Search")
            }
        }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    guard !isBackgroundPressing else { return }

                    isBackgroundPressing = true
                    rippleOrigin = value.location
                    rippleProgress = 0
                    isRippleActive = true
                    rippleID += 1

                    withAnimation(.easeOut(duration: DotGridMetrics.rippleDuration)) {
                        rippleProgress = 1
                    }
                }
                .onEnded { _ in
                    isBackgroundPressing = false
                }
        )
        .task(id: rippleID) {
            guard rippleID > 0 else { return }
            try? await Task.sleep(for: .seconds(DotGridMetrics.rippleDuration))
            guard !Task.isCancelled else { return }
            isRippleActive = false
        }
    }
}

#Preview {
    ContentView()
}
