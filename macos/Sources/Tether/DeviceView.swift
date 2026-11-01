import SwiftUI

private let stateHints = [
    "unauthorized": "Unlock the phone and accept the “Allow USB debugging?” prompt.",
    "offline": "The device is offline. Try reconnecting it.",
]

/// The device page's sections, below the header and controls.
enum DeviceTab: String, CaseIterable {
    case apps = "Apps"
    case notifications = "Notifications"
    case phone = "Phone"

    var systemImage: String {
        switch self {
        case .apps: "square.grid.2x2"
        case .notifications: "bell"
        case .phone: "info.circle"
        }
    }
}

struct DeviceView: View {
    @Environment(AppModel.self) private var model
    let device: Device
    let search: String
    @State private var tab = DeviceTab.apps

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
                    if let info = model.info[device.serial] {
                        ControlsCard(device: device, info: info)
                    }
                    Picker("Section", selection: $tab) {
                        ForEach(DeviceTab.allCases, id: \.self) { tab in
                            Label(title(tab), systemImage: tab.systemImage).tag(tab)
                        }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .fixedSize()
                    .frame(maxWidth: .infinity)
                    switch tab {
                    case .apps:
                        AppsSection(device: device, search: search)
                    case .notifications:
                        NotificationsList(device: device)
                    case .phone:
                        if let info = model.info[device.serial] {
                            DetailsCard(info: info)
                        } else {
                            ProgressView().frame(maxWidth: .infinity, minHeight: 120)
                        }
                    }
                }
                .padding(28)
            }
            .dropDestination(for: URL.self) { urls, _ in
                let apks = urls.filter { $0.pathExtension.lowercased() == "apk" }
                guard !apks.isEmpty else { return false }
                model.install(apks, on: device)
                return true
            }
        }
    }

    private func title(_ tab: DeviceTab) -> String {
        guard tab == .notifications, let count = model.notifications[device.serial]?.count, count > 0 else { return tab.rawValue }
        return "\(tab.rawValue) (\(count))"
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
                Pill(text: device.shortSerial, systemImage: "number")
                    .textSelection(.enabled)
            }
            HStack(spacing: 10) {
                Button("Mirror Screen", systemImage: "rectangle.on.rectangle") { model.mirror(device) }
                    .buttonStyle(.glassProminent)
                    .keyboardShortcut("m", modifiers: [.command, .shift])
                if !device.wireless {
                    Button("Go Wireless", systemImage: "wifi") { model.goWireless(device) }
                        .buttonStyle(.glass)
                }
                Toggle(
                    "Capture Mouse",
                    systemImage: "cursorarrow.rays",
                    isOn: Binding(get: { model.config.captureMouse }, set: { model.setCaptureMouse($0) })
                )
                .toggleStyle(.button)
                .buttonStyle(.glass)
                .help("Use the mouse like a real one on the phone (for games and apps that capture the pointer). Press Left ⌥ or ⌘ to release it.")
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
                WindowSizeButton()
            }
            Text(supported ? "Click an app to open it in its own window." : "App windows need Android 10 or newer.")
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
