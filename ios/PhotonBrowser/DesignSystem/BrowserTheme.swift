import SwiftUI

enum BrowserTheme {
    enum Colors {
        static let homeBase = Color(red: 0.96, green: 0.91, blue: 0.96)
        static let homePinkGlow = Color(red: 0.98, green: 0.76, blue: 0.89)
        static let homeBlueGlow = Color(red: 0.72, green: 0.83, blue: 0.98)
        static let homeLilacGlow = Color(red: 0.84, green: 0.79, blue: 0.96)
        static let homeDot = Color(red: 0.34, green: 0.32, blue: 0.43).opacity(0.14)
        static let homeIcon = Color(red: 0.16, green: 0.15, blue: 0.20)
        static let fallbackGlassBorder = Color.white.opacity(0.58)
        static let fallbackGlassShadow = Color.black.opacity(0.10)
    }

    enum Metrics {
        static let controlDiameter: CGFloat = 68
        static let addControlMaximumWidth: CGFloat = 224
        static let addControlHeight: CGFloat = 72
        static let minimumControlGap: CGFloat = 18
        static let homeHorizontalInset: CGFloat = 26
        static let homeBottomInset: CGFloat = 8
        static let homeControlsHeight: CGFloat = 76
        static let iconSize: CGFloat = 26
        static let addIconSize: CGFloat = 32
        static let dotSpacing: CGFloat = 28
        static let dotDiameter: CGFloat = 4
        static let glassContainerSpacing: CGFloat = 0
        static let glassBorderWidth: CGFloat = 0.8
        static let glassShadowRadius: CGFloat = 12
        static let glassShadowOffset: CGFloat = 5
    }
}
