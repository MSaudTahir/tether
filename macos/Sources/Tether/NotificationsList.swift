import SwiftUI

/// The phone's current notifications; clicking one opens its app.
struct NotificationsList: View {
    @Environment(AppModel.self) private var model
    let device: Device

    var body: some View {
        let all = model.notifications[device.serial] ?? []
        if all.isEmpty {
            ContentUnavailableView("No Notifications", systemImage: "bell.slash", description: Text("Notifications on the phone show up here."))
        } else {
            LazyVStack(spacing: 2) {
                ForEach(all, id: \.key) { notification in
                    NotificationRow(notification: notification) { model.open(notification, on: device) }
                }
            }
            .padding(6)
            .glassEffect(.regular, in: .rect(cornerRadius: 20))
        }
    }
}

private struct NotificationRow: View {
    let notification: PhoneNotification
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 12) {
                AppIcon(app: DeviceApp(name: appName, package: notification.package, system: false, iconPath: notification.iconPath))
                    .frame(width: 32, height: 32)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(appName).font(.caption).foregroundStyle(.secondary)
                        Spacer()
                        if notification.time > 0 {
                            Text(Date(timeIntervalSince1970: Double(notification.time) / 1000), format: .relative(presentation: .named))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    if !notification.title.isEmpty {
                        Text(notification.title).font(.headline).lineLimit(1)
                    }
                    if !notification.text.isEmpty {
                        Text(notification.text).lineLimit(3).foregroundStyle(.secondary)
                    }
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary.opacity(hovering ? 1 : 0), in: .rect(cornerRadius: 14))
            .contentShape(.rect(cornerRadius: 14))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Open \(appName)")
    }

    private var appName: String {
        notification.appName.isEmpty ? notification.package : notification.appName
    }
}
