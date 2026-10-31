import SwiftUI

private let stateHints = [
    "unauthorized": "Unlock the phone and accept the “Allow USB debugging?” prompt.",
    "offline": "The device is offline. Try reconnecting it.",
]

private let windowSizes: [(label: String, size: String)] = [
    ("1280 × 800", "1280x800/200"),
    ("1920 × 1080", "1920x1080/280"),
    ("2560 × 1440", "2560x1440/360"),
    ("Same as Device", ""),
]

struct DeviceView: View {
    @Environment(AppModel.self) private var model
    let device: Device
    let search: String

    var body: some View {
        if device.state != "device" {
            ContentUnavailableView(
                device.label,
                systemImage: "lock.iphone",
                description: Text(stateHints[device.state] ?? "The device is \(device.state).")
            )
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 28) {
                    DeviceHeader(device: device, info: model.info[device.serial])
                    AppsSection(device: device, search: search)
                }
                .padding(28)
            }
        }
    }
}

struct DeviceHeader: View {
    @Environment(AppModel.self) private var model
    let device: Device
    let info: DeviceInfo?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(info?.name ?? device.label)
                .font(.largeTitle.bold())
            if let info {
                Text("\(info.model) · Android \(info.android)")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: 8) {
                if let info {
                    Pill(text: "\(info.battery)% · \(info.power.capitalizedFirst)", systemImage: batterySymbol(info))
                }
                Pill(text: device.wireless ? "Wi-Fi" : "USB", systemImage: device.wireless ? "wifi" : "cable.connector")
                Pill(text: device.serial, systemImage: "number")
                    .textSelection(.enabled)
            }
            HStack(spacing: 10) {
                Button("Mirror Screen", systemImage: "rectangle.on.rectangle") { model.mirror(device) }
                    .buttonStyle(.glassProminent)
                if !device.wireless {
                    Button("Go Wireless", systemImage: "wifi") { model.goWireless(device) }
                        .buttonStyle(.glass)
                }
            }
            .controlSize(.large)
            .padding(.top, 8)
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 24))
    }

    private func batterySymbol(_ info: DeviceInfo) -> String {
        if info.power.hasPrefix("charging") || info.power.hasPrefix("full") { return "battery.100percent.bolt" }
        let level = [0, 25, 50, 75, 100].min { abs($0 - Int(info.battery)) < abs($1 - Int(info.battery)) } ?? 100
        return "battery.\(level)percent"
    }
}

struct AppsSection: View {
    @Environment(AppModel.self) private var model
    let device: Device
    let search: String
    @State private var customSize = ""
    @State private var showCustomSize = false

    private var apps: [DeviceApp] {
        let all = model.apps[device.serial] ?? []
        guard !search.isEmpty else { return all }
        return all.filter { $0.name.localizedCaseInsensitiveContains(search) || $0.package.localizedCaseInsensitiveContains(search) }
    }

    private var supported: Bool {
        (model.info[device.serial]?.sdk ?? UInt32.max) >= virtualDisplayMinSdk()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                Text("Apps").font(.title2.bold())
                if let all = model.apps[device.serial] {
                    Text("\(all.count)").foregroundStyle(.secondary)
                }
                Spacer()
                windowSizeMenu
            }
            Text(supported ? "Click an app to open it in its own window." : "DeviceApp windows need Android 10 or newer.")
                .foregroundStyle(.secondary)

            if model.loadingApps.contains(device.serial) && model.apps[device.serial] == nil {
                ProgressView("Loading apps…")
                    .frame(maxWidth: .infinity, minHeight: 200)
            } else if let error = model.appErrors[device.serial] {
                ContentUnavailableView {
                    Label("Couldn't List Apps", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Try Again") { model.refresh() }
                }
            } else {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 8)], spacing: 8) {
                    ForEach(apps, id: \.package) { app in
                        AppTile(app: app) { model.open(app, on: device) }
                    }
                }
                .disabled(!supported)
            }
        }
        .alert("Custom Window Size", isPresented: $showCustomSize) {
            TextField("1600x1000/240", text: $customSize)
            Button("Cancel", role: .cancel) {}
            Button("Set") { model.setWindowSize(customSize) }
        } message: {
            Text("Width × height and density, as WIDTHxHEIGHT/DPI.")
        }
    }

    private var windowSizeMenu: some View {
        let current = model.config.windowSize
        let label = windowSizes.first { $0.size == current }?.label ?? current
        return Menu {
            ForEach(windowSizes, id: \.size) { option in
                Button(option.label) { model.setWindowSize(option.size) }
            }
            Divider()
            Button("Custom…") {
                customSize = current
                showCustomSize = true
            }
        } label: {
            Label("Window: \(label)", systemImage: "macwindow")
        }
        .menuStyle(.button)
        .buttonStyle(.glass)
        .fixedSize()
        .help("Size of the virtual display apps open on")
    }
}

struct AppTile: View {
    let app: DeviceApp
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            VStack(spacing: 6) {
                AppIcon(app: app)
                    .frame(width: 56, height: 56)
                Text(app.name)
                    .font(.callout)
                    .lineLimit(2)
                    .multilineTextAlignment(.center)
                    .frame(height: 34, alignment: .top)
            }
            .padding(8)
            .frame(maxWidth: .infinity)
            .background(.quaternary.opacity(hovering ? 1 : 0), in: .rect(cornerRadius: 14))
            .contentShape(.rect(cornerRadius: 14))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(app.package)
    }
}
