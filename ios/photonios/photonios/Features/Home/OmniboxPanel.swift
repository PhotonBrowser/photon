import SwiftUI

struct OmniboxPanel: View {
    @Binding var text: String
    @FocusState.Binding var isFocused: Bool

    let showsBorderBeam: Bool
    let onSubmit: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            URLBarInput(
                text: $text,
                isFocused: $isFocused,
                showsBorderBeam: showsBorderBeam,
                onSubmit: onSubmit
            )

            RecentHistorySection()
        }
        .frame(maxWidth: .infinity)
        .background {
            UnevenRoundedRectangle(
                cornerRadii: RectangleCornerRadii(
                    topLeading: URLBarStyle.cardRadius,
                    bottomLeading: 0,
                    bottomTrailing: 0,
                    topTrailing: URLBarStyle.cardRadius
                ),
                style: .continuous
            )
            .fill(.regularMaterial)
            .ignoresSafeArea(edges: .bottom)
        }
    }
}

private struct RecentHistorySection: View {
    var body: some View {
        VStack(alignment: .leading, spacing: URLBarStyle.historyRowSpacing) {
            Text("Recent")
                .font(.headline)
                .foregroundStyle(.primary)

            HStack(spacing: URLBarStyle.historyRowSpacing) {
                Image(systemName: "clock.arrow.circlepath")
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 4) {
                    Text("No recent pages")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(.primary)

                    Text("Pages you visit will appear here.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer(minLength: 0)
            }
            .padding(.vertical, URLBarStyle.historyVerticalInset)
        }
        .padding(.horizontal, URLBarStyle.historyHorizontalInset)
        .padding(.top, URLBarStyle.historyVerticalInset)
        .padding(.bottom, URLBarStyle.historyVerticalInset)
        .frame(
            maxWidth: .infinity,
            minHeight: URLBarStyle.historySectionHeight,
            alignment: .topLeading
        )
    }
}
