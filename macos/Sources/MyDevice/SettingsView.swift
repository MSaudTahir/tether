import SwiftUI

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var choosingScrcpy = false

    var body: some View {
        Form {
            Section("adb") {
                AdbPicker()
            }
            Section("scrcpy") {
                LabeledContent("In use", value: model.core.scrcpyPath() ?? "Not found")
                HStack {
                    if !model.config.scrcpy.isEmpty, model.core.bundledScrcpyVersion() != nil {
                        Button("Use Downloaded scrcpy") { setScrcpy("") }
                    }
                    Button("Choose scrcpy…") { choosingScrcpy = true }
                }
            }
            Section {
                Button("Run Setup Again") { model.runSetupAgain() }
            }
        }
        .formStyle(.grouped)
        .frame(width: 560)
        .fixedSize(horizontal: false, vertical: true)
        .fileImporter(isPresented: $choosingScrcpy, allowedContentTypes: [.unixExecutable, .item]) { result in
            if case .success(let url) = result { setScrcpy(url.path) }
        }
    }

    private func setScrcpy(_ path: String) {
        var config = model.config
        config.scrcpy = path
        model.save(config)
    }
}

/// Lets the user pick which adb to use; saves immediately.
struct AdbPicker: View {
    @Environment(AppModel.self) private var model
    @State private var choosingFile = false

    private static let names = ["studio": "Android Studio", "system": "System", "bundled": "Bundled with scrcpy"]

    var body: some View {
        let candidates = model.core.adbCandidates()
        let current = model.config.adb
        let custom = !["auto", "studio", "system", "bundled"].contains(current)
        VStack(alignment: .leading, spacing: 10) {
            Picker("adb", selection: Binding(get: { current }, set: { setAdb($0) })) {
                choice(title: "Automatic (recommended)", path: model.core.resolveAdb(choice: "auto")).tag("auto")
                ForEach(candidates, id: \.source) { candidate in
                    choice(title: Self.names[candidate.source] ?? candidate.source, path: candidate.path).tag(candidate.source)
                }
                if custom {
                    choice(title: "Custom", path: current).tag(current)
                }
            }
            .pickerStyle(.radioGroup)
            .labelsHidden()
            Button("Choose Another adb…") { choosingFile = true }
        }
        .fileImporter(isPresented: $choosingFile, allowedContentTypes: [.unixExecutable, .item]) { result in
            if case .success(let url) = result { setAdb(url.path) }
        }
    }

    private func choice(title: String, path: String?) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(title)
            Text(path ?? "Not found")
                .font(.caption)
                .foregroundStyle(.secondary)
                .truncationMode(.middle)
                .lineLimit(1)
        }
    }

    private func setAdb(_ choice: String) {
        var config = model.config
        config.adb = choice
        model.save(config)
    }
}
