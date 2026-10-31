import SwiftUI

struct HomeScreen: View {
    var onLibrary: () -> Void = {}
    var onNewPage: () -> Void = {}
    var onSettings: () -> Void = {}

    var body: some View {
        ZStack {
            HomeBackground()
                .ignoresSafeArea()
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            HomeBottomBar(
                onLibrary: onLibrary,
                onNewPage: onNewPage,
                onSettings: onSettings
            )
            .padding(.horizontal, BrowserTheme.Metrics.homeHorizontalInset)
            .padding(.bottom, BrowserTheme.Metrics.homeBottomInset)
        }
    }
}

private struct HomeBackground: View {
    var body: some View {
        GeometryReader { geometry in
            ZStack {
                BrowserTheme.Colors.homeBase

                RadialGradient(
                    colors: [BrowserTheme.Colors.homePinkGlow.opacity(0.76), .clear],
                    center: .topLeading,
                    startRadius: 0,
                    endRadius: geometry.size.width * 1.05
                )

                RadialGradient(
                    colors: [BrowserTheme.Colors.homeBlueGlow.opacity(0.72), .clear],
                    center: .topTrailing,
                    startRadius: 0,
                    endRadius: geometry.size.width * 0.98
                )

                RadialGradient(
                    colors: [BrowserTheme.Colors.homeLilacGlow.opacity(0.36), .clear],
                    center: .bottomTrailing,
                    startRadius: 0,
                    endRadius: geometry.size.width * 0.9
                )

                DotPattern()
            }
        }
        .accessibilityHidden(true)
    }
}

private struct DotPattern: View {
    var body: some View {
        Canvas { context, size in
            let spacing = BrowserTheme.Metrics.dotSpacing
            let diameter = BrowserTheme.Metrics.dotDiameter
            let columns = Int(size.width / spacing) + 1
            let rows = Int(size.height / spacing) + 1

            for row in 0...rows {
                for column in 0...columns {
                    let rect = CGRect(
                        x: CGFloat(column) * spacing - diameter / 2,
                        y: CGFloat(row) * spacing - diameter / 2,
                        width: diameter,
                        height: diameter
                    )
                    context.fill(Path(ellipseIn: rect), with: .color(BrowserTheme.Colors.homeDot))
                }
            }
        }
    }
}
