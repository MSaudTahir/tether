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
                Text("Click a notification to tap it on the phone's screen.")
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
        .help("Show on the phone's screen to tap it")
        .contextMenu {
            Button("Show on Phone") { model.show(notification, on: device) }
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
