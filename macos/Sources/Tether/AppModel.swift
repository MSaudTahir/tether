import AppKit
import Observation

/// DeviceApp state. Core calls block (adb, scrcpy, downloads), so they run off the main actor via `background`.
@MainActor @Observable
final class AppModel {
    static let shared = AppModel()

    let core: Core
    var config: Config
    var needsSetup: Bool

    var devices: [Device] = []
    var selection: String?
    var info: [String: DeviceInfo] = [:]
    var apps: [String: [DeviceApp]] = [:]
    var loadingApps: Set<String> = []
    var appErrors: [String: String] = [:]
    var notifications: [String: [PhoneNotification]] = [:]
    var update: Release?
    var installingUpdate = false
    /// An error to show in an alert.
    var alert: String?

    private var started = false

    private init() {
        let dir = URL.applicationSupportDirectory.appending(path: "io.github.msaudtahir.Tether")
        // Keep scrcpy, settings and the app cache from when the app was called MyDevice.
        let old = URL.applicationSupportDirectory.appending(path: "app.mydevice.MyDevice")
        if !FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.moveItem(at: old, to: dir)
        }
        core = Core(dataDir: dir.path)
        config = core.config()
        needsSetup = core.needsSetup()
    }

    var selectedDevice: Device? { devices.first { $0.serial == selection } }

    /// Starts watching for devices and checks for a scrcpy update.
    func start() {
        guard !started else { return }
        started = true
        Task {
            for tick in 1... {
                await refreshDevices()
                // Keep notifications, battery, memory etc. current for the device on screen.
                if tick % 3 == 0, let serial = selection { await loadNotifications(serial) }
                if tick % 15 == 0, let serial = selection { await loadInfo(serial) }
                try? await Task.sleep(for: .seconds(2))
            }
        }
        checkForUpdate()
        let core = core
        Task { _ = try? await offMain { try core.reconnect() } }  // phones last used over Wi-Fi
    }

    func refreshDevices() async {
        let core = core
        guard let devices = try? await offMain({ try core.devices() }) else { return }
        if devices != self.devices {
            self.devices = devices
        }
        if selection == nil || !devices.contains(where: { $0.serial == selection }) {
            selection = devices.first?.serial
        }
    }

    // MARK: Selected device

    /// Loads info and the (cached) app list for a device.
    func load(_ serial: String) async {
        guard devices.first(where: { $0.serial == serial })?.state == "device" else { return }
        async let info: Void = loadInfo(serial)
        async let apps: Void = loadApps(serial)
        _ = await (info, apps)
        await loadNotifications(serial)  // after the apps, which give them names and icons
        await checkBuddy(serial)
    }

    private func loadNotifications(_ serial: String) async {
        guard devices.first(where: { $0.serial == serial })?.state == "device" else { return }
        let core = core
        if let list = try? await offMain({ try core.notifications(serial: serial) }), list != notifications[serial] {
            notifications[serial] = list
        }
    }

    /// Opens a notification on the phone's screen, mirrored: with Tether Buddy its action runs (e.g.
    /// that chat opens); without, the notification shade is pulled down to tap it.
    func show(_ notification: PhoneNotification, on device: Device) {
        let core = core
        Task {
            _ = try? await offMain { try core.openNotification(serial: device.serial, key: notification.key) }
            mirror(device)
        }
    }

    /// Tether Buddy's state per phone; nil until checked.
    var buddy: [String: BuddyState] = [:]
    var installingBuddy: Set<String> = []

    func checkBuddy(_ serial: String) async {
        let core = core
        if let state = try? await offMain({ try core.buddyState(serial: serial) }) {
            buddy[serial] = state
        }
    }

    func installBuddy(on device: Device) {
        let core = core
        installingBuddy.insert(device.serial)
        Task {
            defer { installingBuddy.remove(device.serial) }
            do {
                try await offMain { try core.installBuddy(serial: device.serial) }
            } catch {
                alert = "Couldn't install Tether Buddy: \(error.localizedDescription)\n\nOn Xiaomi phones, turn on Developer options → Install via USB first."
            }
            await checkBuddy(device.serial)
        }
    }

    /// Opens the notification's app: in its own window if the phone supports that, else mirrored.
    func openApp(of notification: PhoneNotification, on device: Device) {
        let app = apps[device.serial]?.first { $0.package == notification.package }
            ?? DeviceApp(name: notification.appName, package: notification.package, system: false, iconPath: nil)
        if (info[device.serial]?.sdk ?? 0) >= virtualDisplayMinSdk() {
            open(app, on: device)
        } else {
            mirror(device)
        }
    }

    /// Reloads info, and the app list from the device rather than the cache.
    func refresh() {
        guard let serial = selection else { return }
        Task {
            async let info: Void = loadInfo(serial)
            async let apps: Void = loadApps(serial, force: true)
            _ = await (info, apps)
        }
    }

    private func loadInfo(_ serial: String) async {
        let core = core
        if let info = try? await offMain({ try core.deviceInfo(serial: serial) }) {
            self.info[serial] = info
        }
    }

    func loadApps(_ serial: String, force: Bool = false) async {
        guard !loadingApps.contains(serial), force || apps[serial] == nil else { return }
        loadingApps.insert(serial)
        appErrors[serial] = nil
        defer { loadingApps.remove(serial) }
        let core = core
        do {
            apps[serial] = try await offMain { try core.apps(serial: serial, force: force) }
        } catch {
            appErrors[serial] = error.localizedDescription
        }
    }

    // MARK: Actions

    func mirror(_ device: Device) {
        let title = info[device.serial]?.name ?? device.label
        launch { core in try core.mirror(serial: device.serial, title: title) }
    }

    func open(_ app: DeviceApp, on device: Device) {
        let title = "\(app.name) · \(info[device.serial]?.name ?? device.label)"
        let main = NSScreen.main
        let screen = Screen(
            width: UInt32(main?.visibleFrame.width ?? 1280), height: UInt32(main?.visibleFrame.height ?? 800),
            scale: Float(main?.backingScaleFactor ?? 1),
            pixelWindows: false  // scrcpy sizes windows in points on macOS
        )
        launch { core in try core.openApp(serial: device.serial, package: app.package, title: title, screen: screen) }
    }

    /// Saves a screenshot to the Desktop and shows it in Finder.
    func screenshot(_ device: Device) {
        let date = DateFormatter()
        date.dateFormat = "yyyy-MM-dd 'at' HH.mm.ss"  // like macOS screenshots
        let name = "\(info[device.serial]?.name ?? device.label) \(date.string(from: .now))"
        let url = URL.desktopDirectory.appending(path: "Screenshot \(name).png")
        let core = core
        Task {
            do {
                try await offMain { try core.screenshot(serial: device.serial, path: url.path) }
                NSWorkspace.shared.activateFileViewerSelecting([url])
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    /// Serials with an APK install in progress.
    var installing: Set<String> = []

    func install(_ apks: [URL], on device: Device) {
        let core = core
        installing.insert(device.serial)
        Task {
            defer { installing.remove(device.serial) }
            for apk in apks {
                do {
                    try await offMain { try core.installApk(serial: device.serial, path: apk.path) }
                } catch {
                    alert = "Couldn't install \(apk.lastPathComponent): \(error.localizedDescription)"
                }
            }
            await loadApps(device.serial, force: true)
        }
    }

    func press(_ key: DeviceKey, on device: Device) {
        run { core in try core.pressKey(serial: device.serial, key: key) }
        if key == .power {
            Task {
                try? await Task.sleep(for: .milliseconds(600))
                await loadInfo(device.serial)
            }
        }
    }

    func setDarkMode(_ on: Bool, on device: Device) {
        update(device) { core in try core.setDarkMode(serial: device.serial, on: on) }
    }

    func setDoNotDisturb(_ on: Bool, on device: Device) {
        update(device) { core in try core.setDoNotDisturb(serial: device.serial, on: on) }
    }

    // MARK: App actions

    /// Shows the app's page in the phone's Settings, mirrored.
    func showAppInfo(_ app: DeviceApp, on device: Device) {
        let core = core
        Task {
            do {
                try await offMain { try core.showAppInfo(serial: device.serial, package: app.package) }
                mirror(device)
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    func forceStop(_ app: DeviceApp, on device: Device) {
        run { core in try core.forceStop(serial: device.serial, package: app.package) }
    }

    func clearData(_ app: DeviceApp, on device: Device) {
        run { core in try core.clearAppData(serial: device.serial, package: app.package) }
    }

    func uninstall(_ app: DeviceApp, on device: Device) {
        let core = core
        Task {
            do {
                try await offMain { try core.uninstallApp(serial: device.serial, package: app.package) }
                await loadApps(device.serial, force: true)
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    /// Opens a link on the phone, mirrored.
    func openLink(_ link: String, on device: Device) {
        let core = core
        Task {
            do {
                try await offMain { try core.openLink(serial: device.serial, link: link) }
                mirror(device)
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    // MARK: Photos

    var photos: [String: [Photo]] = [:]
    var photosLoading: Set<String> = []

    func loadPhotos(_ device: Device) {
        let (core, serial) = (core, device.serial)
        photosLoading.insert(serial)
        Task {
            defer { photosLoading.remove(serial) }
            do {
                photos[serial] = try await offMain { try core.recentPhotos(serial: serial, count: 60) }
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    func setStayAwake(_ on: Bool, on device: Device) {
        update(device) { core in try core.setStayAwake(serial: device.serial, on: on) }
    }

    func reboot(_ device: Device, mode: RebootMode) {
        run { core in try core.reboot(serial: device.serial, mode: mode) }
    }

    /// Changes a device setting, then reloads its info to show the result.
    private func update(_ device: Device, _ action: @escaping @Sendable (Core) throws -> Void) {
        let core = core
        Task {
            do {
                try await offMain { try action(core) }
            } catch {
                alert = error.localizedDescription
            }
            await loadInfo(device.serial)
        }
    }

    // MARK: Files

    /// The folder shown in Files, per phone.
    var filePath: [String: String] = [:]
    var files: [String: [PhoneFile]] = [:]
    var filesLoading: Set<String> = []
    var filesError: [String: String] = [:]
    /// Copies in progress, per phone.
    var transfers: [String: Int] = [:]
    var showHiddenFiles = false

    func currentFolder(_ device: Device) -> String { filePath[device.serial] ?? phoneStorage() }

    func openFolder(_ path: String, on device: Device) {
        filePath[device.serial] = path
        loadFiles(device)
    }

    func loadFiles(_ device: Device) {
        let (core, serial, path, hidden) = (core, device.serial, currentFolder(device), showHiddenFiles)
        filesLoading.insert(serial)
        Task {
            defer { filesLoading.remove(serial) }
            do {
                files[serial] = try await offMain { try core.listFiles(serial: serial, path: path, hidden: hidden) }
                filesError[serial] = nil
            } catch {
                filesError[serial] = error.localizedDescription
            }
        }
    }

    /// Runs a copy or change on the phone's files, then reloads the folder.
    private func fileAction(_ device: Device, _ work: @escaping @Sendable (Core) throws -> Void, then: (@MainActor () -> Void)? = nil) {
        let core = core
        transfers[device.serial, default: 0] += 1
        Task {
            do {
                try await offMain { try work(core) }
                then?()
            } catch {
                alert = error.localizedDescription
            }
            transfers[device.serial, default: 1] -= 1
            loadFiles(device)
        }
    }

    /// Downloads to the Downloads folder and shows them in Finder.
    func download(_ items: [PhoneFile], from device: Device) {
        let downloads = URL.downloadsDirectory.path
        let serial = device.serial
        let results = ResultBox()
        fileAction(device, { core in
            for item in items {
                results.append(try core.downloadFile(serial: serial, path: item.path, localDir: downloads))
            }
        }, then: {
            NSWorkspace.shared.activateFileViewerSelecting(results.paths.map { URL(filePath: $0) })
        })
    }

    /// Downloads to a temporary folder and opens it with its Mac app.
    func open(_ item: PhoneFile, on device: Device) {
        if item.folder { return openFolder(item.path, on: device) }
        let temp = URL.temporaryDirectory.appending(path: "Tether/\(UUID().uuidString)").path
        let serial = device.serial
        let results = ResultBox()
        fileAction(device, { core in
            results.append(try core.downloadFile(serial: serial, path: item.path, localDir: temp))
        }, then: {
            if let path = results.paths.first { NSWorkspace.shared.open(URL(filePath: path)) }
        })
    }

    func upload(_ urls: [URL], to device: Device) {
        let (serial, folder) = (device.serial, currentFolder(device))
        fileAction(device) { core in
            for url in urls {
                try core.uploadFile(serial: serial, localPath: url.path, folder: folder)
            }
        }
    }

    func makeFolder(named name: String, on device: Device) {
        let (serial, path) = (device.serial, "\(currentFolder(device))/\(name)")
        fileAction(device) { core in try core.makeFolder(serial: serial, path: path) }
    }

    func rename(_ item: PhoneFile, to name: String, on device: Device) {
        let parent = (item.path as NSString).deletingLastPathComponent
        let serial = device.serial
        fileAction(device) { core in try core.renameFile(serial: serial, from: item.path, to: "\(parent)/\(name)") }
    }

    func delete(_ items: [PhoneFile], on device: Device) {
        let (serial, paths) = (device.serial, items.map(\.path))
        fileAction(device) { core in try core.deleteFiles(serial: serial, paths: paths) }
    }

    /// A short result of the last clipboard action, per phone.
    var clipboardStatus: [String: String] = [:]

    /// Puts the Mac's clipboard text on the phone's clipboard.
    func sendClipboard(to device: Device) {
        guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else {
            clipboardStatus[device.serial] = "The Mac's clipboard has no text."
            return
        }
        let core = core
        Task {
            do {
                try await offMain { try core.setPhoneClipboard(serial: device.serial, text: text) }
                clipboardStatus[device.serial] = "Sent \(text.count) characters to the phone."
            } catch {
                clipboardStatus[device.serial] = error.localizedDescription
            }
        }
    }

    /// Copies the phone's clipboard text to the Mac's clipboard.
    func copyClipboard(from device: Device) {
        let core = core
        Task {
            do {
                if let text = try await offMain({ try core.phoneClipboard(serial: device.serial) }) {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                    clipboardStatus[device.serial] = "Copied \(text.count) characters from the phone."
                } else {
                    clipboardStatus[device.serial] = "Nothing to copy: the phone's clipboard is empty, or the phone doesn't let it be read (Xiaomi phones don't)."
                }
            } catch {
                clipboardStatus[device.serial] = error.localizedDescription
            }
        }
    }

    func goWireless(_ device: Device) {
        run { core in _ = try core.goWireless(serial: device.serial) }
    }

    /// Pairs (if a code is given) and/or connects over Wi-Fi. Throws so the sheet can show the error.
    func pair(address: String, code: String, connect: String) async throws {
        let core = core
        try await offMain {
            if !address.isEmpty && !code.isEmpty { try core.pair(address: address, code: code) }
            if !connect.isEmpty { try core.connect(address: connect) }
        }
        await refreshDevices()
    }

    func setWindowSize(_ size: WindowSize) {
        var config = config
        config.windowSize = formatWindowSize(size: size)
        save(config)
    }

    var windowSize: WindowSize {
        parseWindowSize(text: config.windowSize) ?? WindowSize(width: 0, height: 0, dpi: 0)
    }

    func setCaptureMouse(_ on: Bool) {
        var config = config
        config.captureMouse = on
        save(config)
    }

    func save(_ config: Config) {
        do {
            try core.saveConfig(config: config)
            self.config = config
        } catch {
            alert = error.localizedDescription
        }
    }

    func finishSetup() {
        var config = core.config()
        config.setupDone = true
        save(config)
        needsSetup = core.needsSetup()
        apps.removeAll()  // adb or scrcpy may have changed
        checkForUpdate()
    }

    func runSetupAgain() {
        needsSetup = true
    }

    // MARK: scrcpy updates

    func checkForUpdate() {
        let core = core
        Task { update = try? await offMain { try core.availableUpdate() } }
    }

    func installUpdate() {
        guard let release = update else { return }
        installingUpdate = true
        let core = core
        Task {
            defer { installingUpdate = false }
            do {
                try await offMain { try core.installScrcpy(release: release) }
                update = nil
            } catch {
                alert = "Couldn't update scrcpy: \(error.localizedDescription)"
            }
        }
    }

    /// Starts a scrcpy window, brings it to the front and reports how it ended.
    private func launch(_ start: @escaping @Sendable (Core) throws -> Session) {
        let core = core
        Task {
            do {
                let session = try await offMain { try start(core) }
                Task { await bringToFront(pid: pid_t(session.pid())) }
                try await offMain { try session.wait() }
            } catch {
                alert = error.localizedDescription
            }
        }
    }

    /// Windows of processes we launch open behind us, so hand focus to scrcpy once its window appears.
    private func bringToFront(pid: pid_t) async {
        for _ in 0..<50 {  // up to 10 s
            guard kill(pid, 0) == 0 else { return }  // exited (the error is reported by wait())
            if hasWindow(pid), let app = NSRunningApplication(processIdentifier: pid) {
                NSApp.yieldActivation(to: app)
                app.activate(from: .current)
                return
            }
            try? await Task.sleep(for: .milliseconds(200))
        }
    }

    private func hasWindow(_ pid: pid_t) -> Bool {
        let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
        return windows.contains {
            $0[kCGWindowOwnerPID as String] as? pid_t == pid && $0[kCGWindowLayer as String] as? Int == 0
        }
    }

    /// Runs a core action in the background and shows its error, if any.
    private func run(_ action: @escaping @Sendable (Core) throws -> Void) {
        let core = core
        Task {
            do {
                try await offMain { try action(core) }
            } catch {
                alert = error.localizedDescription
            }
        }
    }
}

/// Runs blocking work off the main actor.
func offMain<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
    try await Task.detached(priority: .userInitiated, operation: work).value
}

/// Collects results from background work.
final class ResultBox: @unchecked Sendable {
    private let lock = NSLock()
    private var items: [String] = []

    func append(_ path: String) { lock.withLock { items.append(path) } }
    var paths: [String] { lock.withLock { items } }
}
