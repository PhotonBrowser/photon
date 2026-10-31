import SwiftUI

struct HomeBottomBar: View {
    let onLibrary: () -> Void
    let onNewPage: () -> Void
    let onSettings: () -> Void

    var body: some View {
        GeometryReader { geometry in
            let addWidth = min(
                BrowserTheme.Metrics.addControlMaximumWidth,
                max(
                    0,
                    geometry.size.width
                        - (BrowserTheme.Metrics.controlDiameter * 2)
                        - (BrowserTheme.Metrics.minimumControlGap * 2)
                )
            )
            let controls = controlRow(addWidth: addWidth)

            if #available(iOS 26.0, *) {
                GlassEffectContainer(spacing: BrowserTheme.Metrics.glassContainerSpacing) {
                    controls
                }
            } else {
                controls
            }
        }
        .frame(height: BrowserTheme.Metrics.homeControlsHeight)
    }

    private func controlRow(addWidth: CGFloat) -> some View {
        HStack(spacing: 0) {
            roundButton(
                symbol: "archivebox",
                label: "Library",
                action: onLibrary
            )

            Spacer(minLength: BrowserTheme.Metrics.minimumControlGap)

            Button(action: onNewPage) {
                Image(systemName: "plus")
                    .font(.system(size: BrowserTheme.Metrics.addIconSize, weight: .regular))
                    .foregroundStyle(BrowserTheme.Colors.homeIcon)
                    .frame(width: addWidth, height: BrowserTheme.Metrics.addControlHeight)
                    .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .modifier(LiquidGlassSurface(shape: Capsule()))
            .accessibilityLabel("New page")

            Spacer(minLength: BrowserTheme.Metrics.minimumControlGap)

            roundButton(
                symbol: "gearshape",
                label: "Settings",
                action: onSettings
            )
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func roundButton(
        symbol: String,
        label: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: BrowserTheme.Metrics.iconSize, weight: .regular))
                .foregroundStyle(BrowserTheme.Colors.homeIcon)
                .frame(
                    width: BrowserTheme.Metrics.controlDiameter,
                    height: BrowserTheme.Metrics.controlDiameter
                )
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .modifier(LiquidGlassSurface(shape: Circle()))
        .accessibilityLabel(label)
    }
}
