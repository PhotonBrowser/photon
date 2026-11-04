import SwiftUI

struct SearchButton: View {
    let namespace: Namespace.ID
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(.primary)
                .frame(width: 100, height: 34)
                .contentShape(Rectangle())
                .transition(.blurReplace)
        }
        .buttonStyle(.glass)
        .matchedGeometryEffect(id: "search-url-bar", in: namespace)
        .accessibilityLabel("Search")
    }
}
