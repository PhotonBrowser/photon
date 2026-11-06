import SwiftUI

private struct DotRipple: Identifiable {
    let id = UUID()
    let origin: CGPoint
    let startedAt: Date
}

private enum DotGridMetrics {
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
    let isURLBarActive: Bool
    let onBackgroundTap: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var touchLocation = CGPoint.zero
    @State private var isTouching = false
    @State private var pressID = 0
    @State private var ripples: [DotRipple] = []

    var body: some View {
        backgroundContent
            .contentShape(Rectangle())
            .modifier(
                DotGridBackgroundInteraction(
                    isURLBarActive: isURLBarActive,
                    onTouchChanged: updateTouch,
                    onTouchEnded: finishTouch,
                    onBackgroundTap: onBackgroundTap
                )
            )
            .ignoresSafeArea()
            .accessibilityHidden(true)
            .sensoryFeedback(.impact(weight: .light, intensity: 0.35), trigger: pressID)
            .onChange(of: reduceMotion) { _, shouldReduceMotion in
                if shouldReduceMotion {
                    ripples.removeAll()
                }
            }
    }

    private var backgroundContent: some View {
        GeometryReader { geometry in
            ZStack {
                StaticDotGrid()

                TouchDotHighlight(
                    touchLocation: touchLocation,
                    isTouching: isTouching,
                    globalFrame: geometry.frame(in: .global)
                )

                if !reduceMotion {
                    DotRippleCanvas(
                        ripples: ripples,
                        globalFrame: geometry.frame(in: .global)
                    )
                }
            }
        }
    }

    private func updateTouch(_ location: CGPoint) {
        if !isTouching {
            isTouching = true
            pressID += 1
        }

        touchLocation = location
    }

    private func finishTouch(_ location: CGPoint) {
        touchLocation = location
        isTouching = false
        guard !reduceMotion else { return }

        let ripple = DotRipple(origin: location, startedAt: .now)
        ripples.append(ripple)

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(DotGridMetrics.rippleDuration))
            ripples.removeAll { $0.id == ripple.id }
        }
    }
}

private struct StaticDotGrid: View {
    var body: some View {
        Canvas { context, size in
            var dots = Path()

            for y in stride(from: 0, through: size.height, by: DotGridMetrics.spacing) {
                for x in stride(from: 0, through: size.width, by: DotGridMetrics.spacing) {
                    dots.addEllipse(in: DotGridDrawing.bounds(at: CGPoint(x: x, y: y)))
                }
            }

            context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.opacity)))
        }
    }
}

private struct TouchDotHighlight: View {
    let touchLocation: CGPoint
    let isTouching: Bool
    let globalFrame: CGRect

    var body: some View {
        Canvas { context, size in
            guard isTouching else { return }

            let origin = DotGridDrawing.localPoint(for: touchLocation, in: globalFrame)
            let firstX = ceil(max(0, origin.x - DotGridMetrics.touchRadius) / DotGridMetrics.spacing)
                * DotGridMetrics.spacing
            let lastX = min(size.width, origin.x + DotGridMetrics.touchRadius)
            let firstY = ceil(max(0, origin.y - DotGridMetrics.touchRadius) / DotGridMetrics.spacing)
                * DotGridMetrics.spacing
            let lastY = min(size.height, origin.y + DotGridMetrics.touchRadius)

            guard firstX <= lastX, firstY <= lastY else { return }

            var dots = Path()
            for y in stride(from: firstY, through: lastY, by: DotGridMetrics.spacing) {
                for x in stride(from: firstX, through: lastX, by: DotGridMetrics.spacing) {
                    let point = CGPoint(x: x, y: y)
                    let distance = hypot(x - origin.x, y - origin.y)
                    let intensity = max(0, 1 - distance / DotGridMetrics.touchRadius)
                    guard intensity > 0.05 else { continue }

                    let diameter = DotGridMetrics.diameter
                        * (1 + intensity * (DotGridMetrics.touchDotScale - 1))
                    dots.addEllipse(in: DotGridDrawing.bounds(at: point, diameter: diameter))
                }
            }

            context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.touchOpacity)))
        }
        .allowsHitTesting(false)
    }
}

private struct DotRippleCanvas: View {
    let ripples: [DotRipple]
    let globalFrame: CGRect

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: ripples.isEmpty)) { timeline in
            Canvas { context, size in
                let activeRipples = ripples.compactMap { ripple -> (origin: CGPoint, radius: CGFloat)? in
                    let elapsed = timeline.date.timeIntervalSince(ripple.startedAt)
                    guard elapsed < DotGridMetrics.rippleDuration else { return nil }

                    let origin = DotGridDrawing.localPoint(for: ripple.origin, in: globalFrame)
                    let progress = CGFloat(elapsed / DotGridMetrics.rippleDuration)
                    let radius = DotGridDrawing.farthestCornerDistance(from: origin, in: size) * progress
                    return (origin, radius)
                }

                guard !activeRipples.isEmpty else { return }

                var dots = Path()
                for y in stride(from: 0, through: size.height, by: DotGridMetrics.spacing) {
                    for x in stride(from: 0, through: size.width, by: DotGridMetrics.spacing) {
                        let intensity = activeRipples.reduce(CGFloat.zero) { strongest, ripple in
                            let distance = hypot(x - ripple.origin.x, y - ripple.origin.y)
                            let value = max(
                                0,
                                1 - abs(distance - ripple.radius) / DotGridMetrics.rippleWidth
                            )
                            return max(strongest, value)
                        }

                        guard intensity > 0.05 else { continue }

                        let diameter = DotGridMetrics.diameter
                            * (1 + intensity * (DotGridMetrics.rippleDotScale - 1))
                        dots.addEllipse(
                            in: DotGridDrawing.bounds(at: CGPoint(x: x, y: y), diameter: diameter)
                        )
                    }
                }

                context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.rippleOpacity)))
            }
        }
        .allowsHitTesting(false)
    }
}

private enum DotGridDrawing {
    static func localPoint(for globalPoint: CGPoint, in frame: CGRect) -> CGPoint {
        CGPoint(x: globalPoint.x - frame.minX, y: globalPoint.y - frame.minY)
    }

    static func bounds(at point: CGPoint, diameter: CGFloat = DotGridMetrics.diameter) -> CGRect {
        CGRect(
            x: point.x - diameter / 2,
            y: point.y - diameter / 2,
            width: diameter,
            height: diameter
        )
    }

    static func farthestCornerDistance(from origin: CGPoint, in size: CGSize) -> CGFloat {
        let horizontalDistance = max(abs(origin.x), abs(size.width - origin.x))
        let verticalDistance = max(abs(origin.y), abs(size.height - origin.y))
        return hypot(horizontalDistance, verticalDistance)
    }
}

private struct DotGridBackgroundInteraction: ViewModifier {
    let isURLBarActive: Bool
    let onTouchChanged: (CGPoint) -> Void
    let onTouchEnded: (CGPoint) -> Void
    let onBackgroundTap: () -> Void

    @ViewBuilder
    func body(content: Content) -> some View {
        if isURLBarActive {
            content.onTapGesture(perform: onBackgroundTap)
        } else {
            content.gesture(
                DragGesture(minimumDistance: 0, coordinateSpace: .global)
                    .onChanged { onTouchChanged($0.location) }
                    .onEnded { onTouchEnded($0.location) }
            )
        }
    }
}
