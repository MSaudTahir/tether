// Draws the app icon: a phone showing a little robot head, on an Android-green tile.
// Usage: swift icon/make_icon.swift icon/icon.png
import AppKit

let size = 1024.0
let image = NSImage(size: NSSize(width: size, height: size), flipped: true) { _ in
    let ctx = NSGraphicsContext.current!.cgContext
    func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
        CGColor(red: CGFloat(hex >> 16 & 0xFF) / 255, green: CGFloat(hex >> 8 & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
    func gradient(_ path: CGPath, _ top: UInt32, _ bottom: UInt32, from y0: CGFloat, to y1: CGFloat) {
        ctx.saveGState()
        ctx.addPath(path)
        ctx.clip()
        let g = CGGradient(colorsSpace: nil, colors: [color(top), color(bottom)] as CFArray, locations: [0, 1])!
        ctx.drawLinearGradient(g, start: CGPoint(x: 0, y: y0), end: CGPoint(x: 0, y: y1), options: [])
        ctx.restoreGState()
    }
    func rounded(_ rect: CGRect, _ radius: CGFloat) -> CGPath {
        CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)
    }

    // Tile (the macOS icon grid: 824pt squircle, centred, with a soft shadow).
    let tile = CGRect(x: 100, y: 100, width: 824, height: 824)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: 12), blur: 28, color: color(0x000000, 0.3))
    ctx.addPath(rounded(tile, 185))
    ctx.setFillColor(color(0x2FBF71))
    ctx.fillPath()
    ctx.restoreGState()
    gradient(rounded(tile, 185), 0x5CE69A, 0x179457, from: 100, to: 924)

    // Phone.
    let phone = CGRect(x: 337, y: 196, width: 350, height: 632)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: 18), blur: 36, color: color(0x003318, 0.45))
    ctx.addPath(rounded(phone, 64))
    ctx.setFillColor(color(0x13202A))
    ctx.fillPath()
    ctx.restoreGState()
    let screen = phone.insetBy(dx: 22, dy: 22)
    gradient(rounded(screen, 44), 0x24384A, 0x101A22, from: screen.minY, to: screen.maxY)
    ctx.setFillColor(color(0x000000, 0.8))
    ctx.fillEllipse(in: CGRect(x: 500, y: 236, width: 24, height: 24)) // camera

    // Robot head: a dome with antennae and eyes.
    let green = color(0x3DDC84)
    let center = CGPoint(x: 512, y: 560)
    let radius = 112.0
    ctx.setFillColor(green)
    ctx.move(to: CGPoint(x: center.x - radius, y: center.y))
    ctx.addArc(center: center, radius: radius, startAngle: .pi, endAngle: 0, clockwise: false)
    ctx.closePath()
    ctx.fillPath()
    ctx.setStrokeColor(green)
    ctx.setLineWidth(16)
    ctx.setLineCap(.round)
    for side in [-1.0, 1.0] {
        ctx.move(to: CGPoint(x: center.x + side * 52, y: center.y - 90))
        ctx.addLine(to: CGPoint(x: center.x + side * 84, y: center.y - 146))
        ctx.strokePath()
        ctx.setFillColor(color(0x13202A))
        ctx.fillEllipse(in: CGRect(x: center.x + side * 46 - 13, y: center.y - 58, width: 26, height: 26))
    }
    // A small "window" bar under the head, hinting at the mirrored screen.
    ctx.setFillColor(green.copy(alpha: 0.35)!)
    ctx.addPath(rounded(CGRect(x: center.x - radius, y: center.y + 36, width: radius * 2, height: 26), 13))
    ctx.fillPath()
    ctx.addPath(rounded(CGRect(x: center.x - radius, y: center.y + 82, width: radius * 1.3, height: 26), 13))
    ctx.fillPath()
    return true
}

let rep = NSBitmapImageRep(data: image.tiffRepresentation!)!
rep.size = NSSize(width: size, height: size)
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon.png"
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
