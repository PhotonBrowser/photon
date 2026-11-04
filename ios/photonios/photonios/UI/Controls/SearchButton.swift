import SwiftUI

struct SearchButton: View {
    let action: () -> Void

    var body: some View {
        Button(action: action) {
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
