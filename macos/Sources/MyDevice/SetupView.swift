import SwiftUI

/// First-run setup: get scrcpy, pick an adb, connect a phone.
struct SetupView: View {
    @Environment(AppModel.self) private var model
    @State private var step = Step.scrcpy

    enum Step: Int, CaseIterable {
        case scrcpy, adb, connect
    }

    var body: some View {
        VStack(spacing: 24) {
            HStack(spacing: 8) {
                ForEach(Step.allCases, id: \.self) { s in
                    Capsule()
                        .fill(s.rawValue <= step.rawValue ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary))
                        .frame(width: 36, height: 5)
                }
            }
            Group {
                switch step {
                case .scrcpy: ScrcpyStep { step = .adb }
                case .adb: AdbStep(back: { step = .scrcpy }) { step = .connect }
                case .connect: ConnectStep(back: { step = .adb }) { model.finishSetup() }
                }
            }
            .frame(maxWidth: 520)
            .padding(32)
            .glassEffect(.regular, in: .rect(cornerRadius: 28))
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.background.secondary)
    }
}

private struct StepHeader: View {
    let symbol: String
    let title: String
    let subtitle: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 44))
                .foregroundStyle(.tint)
            Text(title).font(.title.bold())
            Text(subtitle)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }
}

private struct ScrcpyStep: View {
    @Environment(AppModel.self) private var model
    let next: () -> Void
    @State private var downloading = false
    @State private var error: String?
    @State private var ready = false
    @State private var choosingFile = false

    var body: some View {
        VStack(spacing: 20) {
            StepHeader(
                symbol: "rectangle.on.rectangle",
                title: "Get scrcpy",
                subtitle: "MyDevice uses scrcpy to show your phone's screen. It's free and open source."
            )
            Group {
                if downloading {
                    ProgressView("Downloading the latest scrcpy…")
                } else if let error {
                    VStack(spacing: 8) {
                        Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                        Button("Try Again") { Task { await download() } }
                    }
                } else if ready {
                    Label(readyText, systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                }
            }
            .frame(minHeight: 44)

            HStack {
                Button("Use My Own scrcpy…") { choosingFile = true }
                    .buttonStyle(.glass)
                Spacer()
                Button("Continue", action: next)
                    .buttonStyle(.glassProminent)
                    .disabled(!ready || downloading)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .fileImporter(isPresented: $choosingFile, allowedContentTypes: [.unixExecutable, .item]) { result in
            guard case .success(let url) = result else { return }
            var config = model.core.config()
            config.scrcpy = url.path
            model.save(config)
            ready = true
            error = nil
        }
        .task {
            let config = model.core.config()
            if model.core.bundledScrcpyVersion() != nil || !config.scrcpy.isEmpty {
                ready = true
            } else {
                await download()
            }
        }
    }

    private var readyText: String {
        let custom = model.core.config().scrcpy
        if !custom.isEmpty { return "Using \(custom)" }
        return "scrcpy \(model.core.bundledScrcpyVersion() ?? "") is ready."
    }

    private func download() async {
        downloading = true
        error = nil
        defer { downloading = false }
        let core = model.core
        do {
            try await offMain {
                guard let release = try core.latestRelease() else {
                    throw CoreError.Failed(message: "There's no prebuilt scrcpy for this Mac. Install it and choose it below.")
                }
                try core.installScrcpy(release: release)
            }
            var config = core.config()
            config.scrcpy = ""
            model.save(config)
            ready = true
        } catch {
            self.error = error.localizedDescription
        }
    }
}

private struct AdbStep: View {
    let back: () -> Void
    let next: () -> Void

    var body: some View {
        VStack(spacing: 20) {
            StepHeader(
                symbol: "cable.connector",
                title: "Choose adb",
                subtitle: "adb is how your Mac talks to the phone. If you use Android Studio, sharing its adb avoids conflicts."
            )
            AdbPicker()
            HStack {
                Button("Back", action: back).buttonStyle(.glass)
                Spacer()
                Button("Continue", action: next)
                    .buttonStyle(.glassProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
    }
}

private struct ConnectStep: View {
    @Environment(AppModel.self) private var model
    let back: () -> Void
    let finish: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            StepHeader(
                symbol: "smartphone",
                title: "Connect Your Phone",
                subtitle: "You can skip this and connect, or pair over Wi-Fi, later."
            )
            VStack(alignment: .leading, spacing: 10) {
                Label("On the phone, open **Settings → About phone** and tap **Build number** 7 times.", systemImage: "1.circle")
                Label("Go to **Settings → System → Developer options** and turn on **USB debugging**.", systemImage: "2.circle")
                Label("Plug the phone in and tap **Allow** on the “Allow USB debugging?” prompt.", systemImage: "3.circle")
            }
            status
                .font(.headline)
                .frame(maxWidth: .infinity)
            HStack {
                Button("Back", action: back).buttonStyle(.glass)
                Spacer()
                Button("Finish", action: finish)
                    .buttonStyle(.glassProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
    }

    @ViewBuilder private var status: some View {
        if let ready = model.devices.first(where: { $0.state == "device" }) {
            Label("\(ready.label) is connected", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
        } else if let device = model.devices.first {
            Label("Found \(device.label). Tap Allow on the phone.", systemImage: "hand.tap")
        } else {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Waiting for a device…")
            }
        }
    }
}
