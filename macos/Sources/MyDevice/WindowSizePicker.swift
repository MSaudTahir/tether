import SwiftUI

/// Shows the app window size; opens a popover to change it.
struct WindowSizeButton: View {
    @Environment(AppModel.self) private var model
    @State private var showing = false

    var body: some View {
        let size = model.windowSize
        Button {
            showing.toggle()
        } label: {
            Label(describeWindowSize(size: size), systemImage: size.height > size.width ? "rectangle.portrait" : "rectangle")
        }
        .buttonStyle(.glass)
        .help("Size of the virtual display apps open on")
        .popover(isPresented: $showing, arrowEdge: .bottom) {
            WindowSizeForm()
                .environment(model)
        }
    }
}

private struct WindowSizeForm: View {
    @Environment(AppModel.self) private var model
    @State private var custom = ""
    @State private var customError = false

    var body: some View {
        let size = model.windowSize
        let choice = windowChoice(size: size)
        let presets = windowPresets()
        let sameAsPhone = choice.preset?.width == 0

        Form {
            Picker("Size", selection: Binding(get: { choice.preset?.name ?? "" }, set: { name in
                if let preset = presets.first(where: { $0.name == name }) {
                    model.setWindowSize(presetWindowSize(preset: preset, portrait: choice.portrait, textSize: choice.textSize))
                }
            })) {
                ForEach(presets, id: \.name) { preset in
                    if preset.width == 0 {
                        Text(preset.name).tag(preset.name)
                    } else {
                        Text("\(preset.name)  \(Text("\(preset.width) × \(preset.height)").foregroundStyle(.secondary))")
                            .tag(preset.name)
                    }
                }
                if choice.preset == nil {
                    Text("Custom").tag("")
                }
            }
            .pickerStyle(.inline)

            Picker("Orientation", selection: Binding(get: { choice.portrait }, set: { setOrientation(portrait: $0) })) {
                Label("Landscape", systemImage: "rectangle").tag(false)
                Label("Portrait", systemImage: "rectangle.portrait").tag(true)
            }
            .pickerStyle(.segmented)
            .disabled(sameAsPhone || size.width == 0)

            Picker("Text Size", selection: Binding(get: { choice.textSize }, set: { textSize in
                if let preset = choice.preset {
                    model.setWindowSize(presetWindowSize(preset: preset, portrait: choice.portrait, textSize: textSize))
                }
            })) {
                Text("Smaller").tag(TextSize.smaller)
                Text("Default").tag(TextSize.default)
                Text("Larger").tag(TextSize.larger)
            }
            .pickerStyle(.segmented)
            .disabled(choice.preset == nil || sameAsPhone)

            Section {
                HStack {
                    TextField("Custom", text: $custom, prompt: Text("1600x1000/240"))
                        .labelsHidden()
                        .onSubmit(setCustom)
                    Button("Set", action: setCustom)
                }
            } header: {
                Text("Custom")
            } footer: {
                Text(customError
                    ? "Use WIDTHxHEIGHT/DPI, e.g. 1600x1000/240."
                    : "Portrait suits most phone apps. Smaller text fits more on screen.")
                    .foregroundStyle(customError ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
            }
        }
        .formStyle(.grouped)
        .frame(width: 360)
        .fixedSize(horizontal: false, vertical: true)
        .onAppear { custom = model.config.windowSize }
    }

    private func setOrientation(portrait: Bool) {
        let size = model.windowSize
        let choice = windowChoice(size: size)
        if let preset = choice.preset {
            model.setWindowSize(presetWindowSize(preset: preset, portrait: portrait, textSize: choice.textSize))
        } else if portrait != (size.height > size.width) {
            model.setWindowSize(WindowSize(width: size.height, height: size.width, dpi: size.dpi))
        }
        custom = model.config.windowSize
    }

    private func setCustom() {
        guard let size = parseWindowSize(text: custom) else {
            customError = true
            return
        }
        customError = false
        model.setWindowSize(size)
    }
}
