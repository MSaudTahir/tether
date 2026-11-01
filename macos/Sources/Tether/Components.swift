import AppKit
import SwiftUI

struct Pill: View {
    let text: String
    let systemImage: String

    var body: some View {
        Label(text, systemImage: systemImage)
            .font(.callout)
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(.quaternary, in: .capsule)
    }
}

/// The app's cached icon, or a letter on a color derived from its name.
struct AppIcon: View {
    let app: DeviceApp

    var body: some View {
        if let path = app.iconPath, let image = IconCache.image(at: path) {
            Image(nsImage: image)
                .resizable()
                .interpolation(.high)
        } else {
            // A stable hash: String.hashValue changes on every launch.
            let hue = Double(app.package.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) % 360 }) / 360
            RoundedRectangle(cornerRadius: 14)
                .fill(Color(hue: hue, saturation: 0.45, brightness: 0.8).gradient)
                .overlay {
                    Text(app.name.prefix(1).uppercased())
                        .font(.system(size: 26, weight: .bold, design: .rounded))
                        .foregroundStyle(.white)
                }
        }
    }
}

@MainActor
enum IconCache {
    private static var images: [String: NSImage] = [:]

    static func image(at path: String) -> NSImage? {
        if let image = images[path] { return image }
        let image = NSImage(contentsOfFile: path)
        images[path] = image
        return image
    }
}

extension String {
    /// "charging (AC)" → "Charging (AC)"
    var capitalizedFirst: String { prefix(1).uppercased() + dropFirst() }
}
