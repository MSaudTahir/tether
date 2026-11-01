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
            while true {
                await refreshDevices()
                try? await Task.sleep(for: .seconds(2))
            }
        }
        checkForUpdate()
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

    private func loadApps(_ serial: String, force: Bool = false) async {
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
        launch { core in try core.openApp(serial: device.serial, package: app.package, title: title) }
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

