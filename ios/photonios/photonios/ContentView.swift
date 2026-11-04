//
//  ContentView.swift
//  photonios
//
//  Created by Theo Slater on 04/11/2026.
//

import SwiftUI

private enum DotGridMetrics {
    static let spacing: CGFloat = 24
    static let diameter: CGFloat = 2
    static let opacity = 0.14
}

private struct DotGridBackground: View {
    var body: some View {
        Canvas { context, size in
            var dots = Path()

            for y in stride(from: 0, through: size.height, by: DotGridMetrics.spacing) {
                for x in stride(from: 0, through: size.width, by: DotGridMetrics.spacing) {
                    let rect = CGRect(
                        x: x - DotGridMetrics.diameter / 2,
                        y: y - DotGridMetrics.diameter / 2,
                        width: DotGridMetrics.diameter,
                        height: DotGridMetrics.diameter
                    )
                    dots.addEllipse(in: rect)
                }
            }

            context.fill(dots, with: .color(Color.primary.opacity(DotGridMetrics.opacity)))
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }
}

struct ContentView: View {
    var body: some View {
        ZStack {
            Color(uiColor: .systemBackground)
                .ignoresSafeArea()

            DotGridBackground()

            VStack {
                Spacer()

                Button {} label: {
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
    }
}

#Preview {
    ContentView()
}
