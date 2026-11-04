import SwiftUI

struct DotRipple: Identifiable {
    let id = UUID()
    let origin: CGPoint
    let startedAt: Date
}

enum DotGridMetrics {
    static let spacing: CGFloat = 24
    static let diameter: CGFloat = 2
    static let opacity = 0.14
    static let rippleWidth: CGFloat = 48
    static let rippleDotScale: CGFloat = 2.4
    static let rippleOpacity = 0.28
    static let rippleDuration = 0.8
    static let touchRadius: CGFloat = 36
    static let touchDotScale: CGFloat = 1.35
    static let touchOpacity = 0.18
}

struct DotGridBackground: View {
    let touchLocation: CGPoint
    let ripples: [DotRipple]
    let isTouching: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 60, paused: ripples.isEmpty)) { timeline in
            GeometryReader { geometry in
                Canvas { context, size in
                    let canvasFrame = geometry.frame(in: .global)
                    let touchOrigin = localPoint(for: touchLocation, in: canvasFrame)
                    let rippleStates = ripples.compactMap { ripple -> (origin: CGPoint, radius: CGFloat)? in
                        let elapsed = timeline.date.timeIntervalSince(ripple.startedAt)
                        guard elapsed < DotGridMetrics.rippleDuration else { return nil }

                        let origin = localPoint(for: ripple.origin, in: canvasFrame)
                        let progress = CGFloat(elapsed / DotGridMetrics.rippleDuration)
                        let radius = farthestCornerDistance(from: origin, in: size) * progress
                        return (origin, radius)
                    }

                    var dots = Path()
                    var touchDots = Path()
                    var rippleDots = Path()

                    for y in stride(from: 0, through: size.height, by: DotGridMetrics.spacing) {
                        for x in stride(from: 0, through: size.width, by: DotGridMetrics.spacing) {
                            let point = CGPoint(x: x, y: y)
                            dots.addEllipse(in: dotBounds(at: point))

                            if isTouching {
                                let distance = hypot(x - touchOrigin.x, y - touchOrigin.y)
                                let intensity = max(0, 1 - distance / DotGridMetrics.touchRadius)

                                if intensity > 0.05 {
                                    let diameter = DotGridMetrics.diameter
                                        * (1 + intensity * (DotGridMetrics.touchDotScale - 1))
                                    touchDots.addEllipse(in: dotBounds(at: point, diameter: diameter))
                                }
                            }

                            let rippleIntensity = rippleStates.reduce(CGFloat.zero) { strongest, ripple in
                                let distance = hypot(x - ripple.origin.x, y - ripple.origin.y)
                                let intensity = max(
                                    0,
                                    1 - abs(distance - ripple.radius) / DotGridMetrics.rippleWidth
                                )
                                return max(strongest, intensity)
                            }

                            if rippleIntensity > 0.05 {
                                let diameter = DotGridMetrics.diameter
                                    * (1 + rippleIntensity * (DotGridMetrics.rippleDotScale - 1))
                                rippleDots.addEllipse(in: dotBounds(at: point, diameter: diameter))
                            }
                        }
                    }

                    context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.opacity)))
                    context.fill(
                        touchDots,
                        with: .color(Color.primary.opacity(DotGridMetrics.touchOpacity))
                    )
                    context.fill(
                        rippleDots,
                        with: .color(Color.primary.opacity(DotGridMetrics.rippleOpacity))
                    )
                }
            }
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }

    private func localPoint(for globalPoint: CGPoint, in frame: CGRect) -> CGPoint {
        CGPoint(x: globalPoint.x - frame.minX, y: globalPoint.y - frame.minY)
    }

    private func dotBounds(at point: CGPoint, diameter: CGFloat = DotGridMetrics.diameter) -> CGRect {
        CGRect(
            x: point.x - diameter / 2,
            y: point.y - diameter / 2,
            width: diameter,
            height: diameter
        )
    }

    private func farthestCornerDistance(from origin: CGPoint, in size: CGSize) -> CGFloat {
        [
            CGPoint.zero,
            CGPoint(x: size.width, y: 0),
            CGPoint(x: 0, y: size.height),
            CGPoint(x: size.width, y: size.height),
        ]
        .map { hypot($0.x - origin.x, $0.y - origin.y) }
        .max() ?? 0
    }
}
