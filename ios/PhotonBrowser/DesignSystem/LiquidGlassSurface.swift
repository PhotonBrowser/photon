import SwiftUI

struct LiquidGlassSurface<Surface: Shape>: ViewModifier {
    let shape: Surface

    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content.glassEffect(.regular.interactive(), in: shape)
        } else {
            content
                .background(.ultraThinMaterial, in: shape)
                .overlay {
                    shape.stroke(
                        BrowserTheme.Colors.fallbackGlassBorder,
                        lineWidth: BrowserTheme.Metrics.glassBorderWidth
                    )
                }
                .shadow(
                    color: BrowserTheme.Colors.fallbackGlassShadow,
                    radius: BrowserTheme.Metrics.glassShadowRadius,
                    x: 0,
                    y: BrowserTheme.Metrics.glassShadowOffset
                )
        }
    }
}
