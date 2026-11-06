import SwiftUI

struct SearchButton: View {
    let action: () -> Void

    @State private var activationID = 0

    var body: some View {
        Button {
            activationID += 1
            action()
        } label: {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(.primary)
                .frame(width: 100, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.glass)
        .accessibilityLabel("Search")
        .sensoryFeedback(.impact(weight: .light, intensity: 0.35), trigger: activationID)
    }
}
