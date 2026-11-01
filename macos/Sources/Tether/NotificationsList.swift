import SwiftUI

/// The phone's current notifications, as separate cards like the phone shows them.
struct NotificationsList: View {
    @Environment(AppModel.self) private var model
    let device: Device

    var body: some View {
        let all = model.notifications[device.serial] ?? []
        if all.isEmpty {
            ContentUnavailableView("No Notifications", systemImage: "bell.slash", description: Text("Notifications on the phone show up here."))
        } else {
            VStack(alignment: .leading, spacing: 10) {
                BuddyBanner(device: device)
                Text(model.buddy[device.serial] == .ready
                    ? "Click a notification to open it on the phone's screen."
                    : "Click a notification to show it on the phone's screen and tap it there.")
                    .foregroundStyle(.secondary)
                LazyVStack(spacing: 10) {
                    ForEach(all, id: \.key) { notification in
                        NotificationCard(notification: notification, device: device)
                    }
                }
            }
            .frame(maxWidth: 720)
            .frame(maxWidth: .infinity)
        }
    }
}

/// Offers Tether Buddy, which lets a click run the notification's own action.
private struct BuddyBanner: View {
    @Environment(AppModel.self) private var model
    let device: Device

    var body: some View {
        if let state = model.buddy[device.serial], state != .ready {
            HStack(spacing: 14) {
                Image(systemName: "sparkles")
                    .font(.title2)
                    .foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text(state == .outdated ? "Update Tether Buddy" : "Open notifications directly with Tether Buddy")
                        .font(.headline)
                    Text("A tiny app with no screens that Tether puts on the phone. With it, clicking a notification does what tapping it does, like opening that chat.")
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Button(title(state)) { model.installBuddy(on: device) }
                    .buttonStyle(.glassProminent)
                    .disabled(model.installingBuddy.contains(device.serial))
            }
            .padding(16)
            .glassEffect(.regular, in: .rect(cornerRadius: 18))
        }
    }

    private func title(_ state: BuddyState) -> String {
        if model.installingBuddy.contains(device.serial) { return "Installing…" }
        switch state {
        case .notInstalled: return "Install"
        case .outdated: return "Update"
        case .disabled: return "Turn On"
        case .ready: return ""
        }
    }
}

private struct NotificationCard: View {
    @Environment(AppModel.self) private var model
    let notification: PhoneNotification
    let device: Device
    @State private var hovering = false

    var body: some View {
        Button {
            model.show(notification, on: device)
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    AppIcon(app: DeviceApp(name: appName, package: notification.package, system: false, iconPath: notification.iconPath))
                        .frame(width: 20, height: 20)
                    Text(appName)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if notification.time > 0 {
                        Text("·").foregroundStyle(.secondary)
                        Text(Date(timeIntervalSince1970: Double(notification.time) / 1000), format: .relative(presentation: .named))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    if hovering {
                        Button("Open App") { model.openApp(of: notification, on: device) }
                            .controlSize(.small)
                            .buttonStyle(.glass)
                    }
                }
                if !notification.title.isEmpty {
                    Text(notification.title).font(.headline).lineLimit(1)
                }
                if !notification.text.isEmpty {
                    Text(notification.text).lineLimit(4)
                }
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 18))
            .contentShape(.rect(cornerRadius: 18))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Open on the phone's screen")
        .contextMenu {
            Button("Open on Phone") { model.show(notification, on: device) }
            Button("Open \(appName)") { model.openApp(of: notification, on: device) }
            Divider()
            Button("Copy Text") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString([notification.title, notification.text].filter { !$0.isEmpty }.joined(separator: "\n"), forType: .string)
            }
        }
    }

    private var appName: String {
        notification.appName.isEmpty ? notification.package : notification.appName
    }
}
