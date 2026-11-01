import SwiftUI

@main
struct TetherApp: App {
    @NSApplicationDelegateAdaptor private var delegate: AppDelegate
    @State private var model = AppModel.shared

    var body: some Scene {
        Window("Tether", id: "main") {
            Group {
                if model.needsSetup {
                    SetupView()
                } else {
                    ContentView()
                }
            }
            .environment(model)
            .frame(minWidth: 820, minHeight: 560)
            .task { model.start() }
        }

        Settings {
            SettingsView()
                .environment(model)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    func applicationWillTerminate(_ notification: Notification) {
        AppModel.shared.core.stopSessions()  // close the scrcpy windows we opened
    }
}
