import SwiftUI

struct ContentView: View {
    @Environment(AppModel.self) private var model
    @State private var search = ""
    @State private var showPair = false

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            List(selection: $model.selection) {
                Section("Devices") {
                    ForEach(model.devices, id: \.serial) { device in
                        DeviceRow(device: device, info: model.info[device.serial])
                            .tag(device.serial)
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: 240)
        } detail: {
            if let device = model.selectedDevice {
                DeviceView(device: device, search: search)
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
}

struct DeviceRow: View {
    let device: Device
    let info: DeviceInfo?

    var body: some View {
        Label {
            VStack(alignment: .leading, spacing: 2) {
                Text(info?.name ?? device.label)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        } icon: {
            Image(systemName: "smartphone")
        }
        .padding(.vertical, 3)
    }

    private var subtitle: String {
        let transport = device.wireless ? "Wi-Fi" : "USB"
        if device.state != "device" { return "\(transport) · \(device.state)" }
        if let info { return "\(transport) · \(info.battery)%" }
        return transport
    }
}
