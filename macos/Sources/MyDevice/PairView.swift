import SwiftUI

struct PairView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var pairAddress = ""
    @State private var code = ""
    @State private var connectAddress = ""
    @State private var working = false
    @State private var error: String?

    var body: some View {
        Form {
            Section {
                TextField("Pairing IP:port", text: $pairAddress, prompt: Text("192.168.1.20:37099"))
                TextField("Pairing code", text: $code, prompt: Text("123456"))
            } header: {
                Text("Pair")
            } footer: {
                Text("On the phone: Developer options → Wireless debugging → Pair device with pairing code. The phone and this Mac must be on the same network.")
                    .foregroundStyle(.secondary)
            }
            Section {
                TextField("IP:port", text: $connectAddress, prompt: Text("192.168.1.20:41235"))
            } header: {
                Text("Connect")
            } footer: {
                Text("The IP address & port shown on the Wireless debugging screen. Already paired? Just fill this in.")
                    .foregroundStyle(.secondary)
            }
            if let error {
                Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
            }
        }
        .formStyle(.grouped)
        .frame(width: 460)
        .navigationTitle("Pair over Wi-Fi")
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button(working ? "Connecting…" : "Connect") { Task { await submit() } }
                    .disabled(working || (connectAddress.isEmpty && (pairAddress.isEmpty || code.isEmpty)))
            }
        }
    }

    private func submit() async {
        working = true
        error = nil
        defer { working = false }
        do {
            try await model.pair(
                address: pairAddress.trimmed, code: code.trimmed, connect: connectAddress.trimmed
            )
            dismiss()
        } catch {
            self.error = error.localizedDescription
        }
    }
}

private extension String {
    var trimmed: String { trimmingCharacters(in: .whitespaces) }
}
