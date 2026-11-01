import SwiftUI

struct ContentView: View {
    @Environment(AppModel.self) private var model
    @State private var search = ""
    @State private var showPair = false
    @State private var section: DeviceTab? = .apps

    var body: some View {
        NavigationSplitView {
            // Like System Settings: the phone up top, its sections below.
            List(selection: $section) {
                DevicePicker(showPair: $showPair)
                    .listRowSeparator(.hidden)
                    .padding(.bottom, 6)
                if model.selectedDevice?.state == "device" {
                    Section {
                        ForEach(DeviceTab.allCases, id: \.self) { tab in
                            Label(tab.rawValue, systemImage: tab.systemImage)
                                .badge(badge(tab))
                                .tag(tab)
                        }
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 210, ideal: 240)
        } detail: {
            if let device = model.selectedDevice {
                DeviceView(device: device, search: search, tab: section ?? .apps)
                    .task(id: device.serial + device.state) { await model.load(device.serial) }
            } else {
                ContentUnavailableView {
                    Label("No Devices", systemImage: "smartphone")
                } description: {
                    Text("Plug in a phone with USB debugging on, or pair one over Wi-Fi.")
                } actions: {
                    Button("Pair over Wi-Fi…") { showPair = true }
                }
            }
        }
        .searchable(text: $search, placement: .toolbar, prompt: "Filter Apps")
        .toolbar {
            if let update = model.update {
                ToolbarItem {
                    Button {
                        model.installUpdate()
                    } label: {
                        if model.installingUpdate {
                            ProgressView().controlSize(.small)
                        } else {
                            Label("Update scrcpy to \(update.version)", systemImage: "arrow.down.circle")
                        }
                    }
                    .help("Update scrcpy to \(update.version)")
                    .disabled(model.installingUpdate)
                }
            }
            ToolbarItem {
                Button("Pair over Wi-Fi", systemImage: "wifi") { showPair = true }
                    .help("Pair over Wi-Fi")
            }
            ToolbarItem {
                Button("Refresh", systemImage: "arrow.clockwise") { model.refresh() }
                    .help("Reload device info and the app list")
                    .keyboardShortcut("r")
                    .disabled(model.selectedDevice?.state != "device")
            }
        }
        .sheet(isPresented: $showPair) { PairView() }
        .alert("Something Went Wrong", isPresented: .constant(model.alert != nil)) {
            Button("OK") { model.alert = nil }
        } message: {
            Text(model.alert ?? "")
        }
    }

    private func badge(_ tab: DeviceTab) -> Int {
        guard tab == .notifications, let serial = model.selection else { return 0 }
        return model.notifications[serial]?.count ?? 0
    }
}

/// The selected phone; a menu switches to another one or pairs a new one.
struct DevicePicker: View {
    @Environment(AppModel.self) private var model
    @Binding var showPair: Bool

    var body: some View {
        Menu {
            ForEach(model.devices, id: \.serial) { device in
                Button {
                    model.selection = device.serial
                } label: {
                    Label(model.info[device.serial]?.name ?? device.label, systemImage: device.serial == model.selection ? "checkmark" : "smartphone")
                }
            }
            if !model.devices.isEmpty { Divider() }
            Button("Pair over Wi-Fi…", systemImage: "wifi") { showPair = true }
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "smartphone")
                    .font(.title2)
                    .foregroundStyle(.tint)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline).lineLimit(1)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .contentShape(.rect)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .help("Switch phones")
    }

    private var title: String {
        guard let device = model.selectedDevice else { return "No Phone" }
        return model.info[device.serial]?.name ?? device.label
    }

    private var subtitle: String {
        guard let device = model.selectedDevice else { return "Connect or pair a phone" }
        let transport = device.wireless ? "Wi-Fi" : "USB"
        if device.state != "device" { return "\(transport) · \(device.state)" }
        if let info = model.info[device.serial] { return "\(transport) · \(info.battery)%" }
        return transport
    }
}
