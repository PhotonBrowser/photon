import SwiftUI

enum BrowserTheme {
    enum Colors {
        static let homeBase = Color(red: 0.96, green: 0.91, blue: 0.96)
        static let homePinkGlow = Color(red: 0.98, green: 0.76, blue: 0.89)
        static let homeBlueGlow = Color(red: 0.72, green: 0.83, blue: 0.98)
        static let homeDot = Color(red: 0.34, green: 0.32, blue: 0.43).opacity(0.14)
        static let homeIcon = Color(red: 0.16, green: 0.15, blue: 0.20)
        static let fallbackGlassBorder = Color.white.opacity(0.58)
        static let fallbackGlassShadow = Color.black.opacity(0.10)
    }

    enum Metrics {
        static let controlDiameter: CGFloat = 50
        static let addControlMaximumWidth: CGFloat = 120
        static let addControlHeight: CGFloat = 52
        static let minimumControlGap: CGFloat = 18
        static let homeHorizontalInset: CGFloat = 24
        static let homeBottomInset: CGFloat = 4
        static let homeControlsHeight: CGFloat = 60
        static let iconSize: CGFloat = 22
        static let addIconSize: CGFloat = 27
        static let dotSpacing: CGFloat = 20
        static let dotDiameter: CGFloat = 3
        static let glassContainerSpacing: CGFloat = 0
        static let glassBorderWidth: CGFloat = 0.8
        static let glassShadowRadius: CGFloat = 12
        static let glassShadowOffset: CGFloat = 5
    }
}
