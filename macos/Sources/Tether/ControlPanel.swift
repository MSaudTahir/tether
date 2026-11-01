import SwiftUI
import UniformTypeIdentifiers

/// Storage, memory, battery and hardware details.
struct DetailsCard: View {
    let info: DeviceInfo

    var body: some View {
        HStack(alignment: .top, spacing: 40) {
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 16, verticalSpacing: 12) {
                if info.storageTotal > 0 {
                    Meter(
                        title: "Storage", systemImage: "internaldrive",
                        used: info.storageUsed, total: info.storageTotal,
                        text: "\(bytes(info.storageTotal - info.storageUsed)) free of \(bytes(info.storageTotal))"
                    )
                }
                if info.memoryTotal > 0 {
                    Meter(
                        title: "Memory", systemImage: "memorychip",
                        used: info.memoryTotal - info.memoryAvailable, total: info.memoryTotal,
                        text: "\(bytes(info.memoryAvailable)) available of \(bytes(info.memoryTotal))"
                    )
                }
                Row(title: "Battery", systemImage: "battery.75percent", value: battery)
            }
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 16, verticalSpacing: 12) {
                if !info.chipset.isEmpty {
                    Row(title: "Chipset", systemImage: "cpu", value: info.chipset)
                }
                if !info.screen.isEmpty {
                    Row(title: "Display", systemImage: "iphone", value: info.screen)
                }
                Row(title: "Android", systemImage: "shield.lefthalf.filled", value: android)
                if info.uptimeSeconds > 0 {
                    Row(
                        title: "Up for", systemImage: "clock",
                        value: Duration.seconds(info.uptimeSeconds).formatted(.units(allowed: [.days, .hours, .minutes], width: .wide, maximumUnitCount: 2))
                    )
                }
                if let address = info.wifiAddress {
                    Row(title: "Wi-Fi address", systemImage: "wifi", value: address)
                }
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 20))
    }

    private var battery: String {
        var parts = ["\(info.battery)%", info.power.capitalizedFirst]
        if !info.batteryHealth.isEmpty { parts.append("\(info.batteryHealth.capitalizedFirst) health") }
        if info.batteryTemperature > 0 { parts.append(String(format: "%.1f °C", info.batteryTemperature)) }
        return parts.joined(separator: " · ")
    }

    private var android: String {
        guard let patch = try? Date(info.securityPatch, strategy: .iso8601.year().month().day()) else {
            return "Android \(info.android)"
        }
        return "Android \(info.android) · Security patch \(patch.formatted(.dateTime.month(.wide).year()))"
    }

    private func bytes(_ count: UInt64) -> String {
        count.formatted(.byteCount(style: .file))
    }
}

private struct Row: View {
    let title: String
    let systemImage: String
    let value: String

    var body: some View {
        GridRow {
            Label(title, systemImage: systemImage)
                .foregroundStyle(.secondary)
            Text(value)
                .textSelection(.enabled)
        }
    }
}

private struct Meter: View {
    let title: String
    let systemImage: String
    let used: UInt64
    let total: UInt64
    let text: String

    var body: some View {
        GridRow {
            Label(title, systemImage: systemImage)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 4) {
                ProgressView(value: Double(used), total: Double(total))
                    .tint(Double(used) / Double(total) > 0.9 ? .orange : .accentColor)
                    .frame(width: 240)
                Text(text)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

/// Buttons and switches that act on the phone without opening a window.
struct ControlsCard: View {
    @Environment(AppModel.self) private var model
    let device: Device
    let info: DeviceInfo
    @State private var choosingApk = false
    @State private var confirmReboot: RebootMode?

    var body: some View {
        // One row when there's room, two otherwise.
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                actions
                Divider().frame(height: 20)
                keys
                Divider().frame(height: 20)
                toggles
                Spacer(minLength: 0)
                restart
            }
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    actions
                    Spacer(minLength: 0)
                    restart
                }
                HStack(spacing: 8) {
                    keys
                    Spacer(minLength: 0)
                    toggles
                }
            }
        }
        .toggleStyle(.button)
        .buttonStyle(.glass)
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 20))
        .fileImporter(isPresented: $choosingApk, allowedContentTypes: [.apk], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { model.install(urls, on: device) }
        }
        .confirmationDialog(
            "Restart \(info.name)?", isPresented: .constant(confirmReboot != nil), presenting: confirmReboot
        ) { mode in
            Button("Restart", role: .destructive) {
                model.reboot(device, mode: mode)
                confirmReboot = nil
            }
            Button("Cancel", role: .cancel) { confirmReboot = nil }
        } message: { mode in
            switch mode {
            case .normal: Text("The phone will disconnect while it restarts.")
            case .recovery: Text("The phone will start in recovery mode. Only do this if you know how to use it.")
            case .bootloader: Text("The phone will start in its bootloader. Only do this if you know how to use it.")
            }
        }
    }

    @ViewBuilder private var actions: some View {
        Button("Screenshot", systemImage: "camera") { model.screenshot(device) }
            .help("Save a screenshot to the Desktop")
        Button(model.installing.contains(device.serial) ? "Installing…" : "Install APK…", systemImage: "arrow.down.app") {
            choosingApk = true
        }
        .disabled(model.installing.contains(device.serial))
        .help("Install an app from an .apk file. You can also drop .apk files on this window.")
    }

    @ViewBuilder private var keys: some View {
        key(.back, "Back", "chevron.backward")
        key(.home, "Home", "circle")
        key(.recentApps, "Recent Apps", "square.on.square")
        key(.volumeDown, "Volume Down", "speaker.minus")
        key(.volumeUp, "Volume Up", "speaker.plus")
        key(.power, info.screenOn ? "Lock Screen" : "Wake Screen", info.screenOn ? "lock" : "sun.max")
    }

    @ViewBuilder private var toggles: some View {
        Toggle("Dark Mode", systemImage: "moon", isOn: Binding(
            get: { info.darkMode }, set: { model.setDarkMode($0, on: device) }
        ))
        Toggle("Stay Awake", systemImage: "cup.and.heat.waves", isOn: Binding(
            get: { info.stayAwake }, set: { model.setStayAwake($0, on: device) }
        ))
        .help("Keep the screen on while charging")
    }

    private var restart: some View {
        Menu("Restart", systemImage: "restart") {
            Button("Restart") { confirmReboot = .normal }
            Button("Restart to Recovery") { confirmReboot = .recovery }
            Button("Restart to Bootloader") { confirmReboot = .bootloader }
        }
        .fixedSize()
    }

    private func key(_ key: DeviceKey, _ title: String, _ systemImage: String) -> some View {
        Button(title, systemImage: systemImage) { model.press(key, on: device) }
            .labelStyle(.iconOnly)
            .help(title)
    }
}

extension UTType {
    static let apk = UTType(filenameExtension: "apk", conformingTo: .data) ?? .data
}
