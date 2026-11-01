import SwiftUI

/// Browse the phone's storage: open, download, upload, rename, delete.
struct FilesView: View {
    @Environment(AppModel.self) private var model
    let device: Device
    @State private var selection = Set<String>()
    @State private var sort = [KeyPathComparator(\PhoneFile.name)]
    @State private var choosingUpload = false
    @State private var newFolder: String?
    @State private var renaming: PhoneFile?
    @State private var renameTo = ""
    @State private var deleting: [PhoneFile] = []

    var body: some View {
        @Bindable var model = model
        let items = (model.files[device.serial] ?? []).sorted { a, b in
            // Folders first, then the chosen column.
            a.folder != b.folder ? a.folder : sort.compare(a, b) == .orderedAscending
        }
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Button("Up", systemImage: "chevron.up") { model.openFolder(parent, on: device) }
                    .labelStyle(.iconOnly)
                    .disabled(folder == "/")
                    .help("Up one folder")
                Breadcrumbs(path: folder) { model.openFolder($0, on: device) }
                Spacer()
                if model.transfers[device.serial, default: 0] > 0 || model.filesLoading.contains(device.serial) {
                    ProgressView().controlSize(.small)
                }
                Toggle("Hidden Files", systemImage: "eye", isOn: $model.showHiddenFiles)
                    .toggleStyle(.button)
                    .labelStyle(.iconOnly)
                    .help("Show hidden files")
                    .onChange(of: model.showHiddenFiles) { model.loadFiles(device) }
                Button("New Folder", systemImage: "folder.badge.plus") { newFolder = "" }
                    .labelStyle(.iconOnly)
                    .help("New folder")
                Button("Upload…", systemImage: "square.and.arrow.up") { choosingUpload = true }
                    .help("Copy files from this Mac to this folder. You can also drop files here.")
                Button("Refresh", systemImage: "arrow.clockwise") { model.loadFiles(device) }
                    .labelStyle(.iconOnly)
                    .help("Reload this folder")
            }
            .buttonStyle(.glass)
            .padding(.horizontal, 20)
            .padding(.vertical, 12)

            if let error = model.filesError[device.serial] {
                ContentUnavailableView("Can't Open This Folder", systemImage: "folder.badge.questionmark", description: Text(error))
            } else {
                table(items)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            model.upload(urls, to: device)
            return !urls.isEmpty
        }
        .fileImporter(isPresented: $choosingUpload, allowedContentTypes: [.item, .folder], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { model.upload(urls, to: device) }
        }
        .alert("New Folder", isPresented: .constant(newFolder != nil)) {
            TextField("Name", text: Binding(get: { newFolder ?? "" }, set: { newFolder = $0 }))
            Button("Create") {
                if let name = newFolder?.trimmingCharacters(in: .whitespaces), !name.isEmpty { model.makeFolder(named: name, on: device) }
                newFolder = nil
            }
            Button("Cancel", role: .cancel) { newFolder = nil }
        }
        .alert("Rename", isPresented: .constant(renaming != nil)) {
            TextField("Name", text: $renameTo)
            Button("Rename") {
                let name = renameTo.trimmingCharacters(in: .whitespaces)
                if let item = renaming, !name.isEmpty, name != item.name { model.rename(item, to: name, on: device) }
                renaming = nil
            }
            Button("Cancel", role: .cancel) { renaming = nil }
        }
        .confirmationDialog(
            deleting.count == 1 ? "Delete “\(deleting[0].name)”?" : "Delete \(deleting.count) items?",
            isPresented: .constant(!deleting.isEmpty)
        ) {
            Button("Delete", role: .destructive) {
                model.delete(deleting, on: device)
                deleting = []
            }
            Button("Cancel", role: .cancel) { deleting = [] }
        } message: {
            Text("They're deleted from the phone right away; this can't be undone.")
        }
        .task(id: device.serial) {
            if model.files[device.serial] == nil { model.loadFiles(device) }
        }
        .onChange(of: folder) { selection.removeAll() }
    }

    private func table(_ items: [PhoneFile]) -> some View {
        Table(items, selection: $selection, sortOrder: $sort) {
            TableColumn("Name", value: \.name) { (item: PhoneFile) in
                Label(item.name, systemImage: icon(item))
            }
            TableColumn("Size", value: \.size) { (item: PhoneFile) in
                SizeCell(item: item)
            }
            .width(90)
            TableColumn("Modified", value: \.modified) { (item: PhoneFile) in
                DateCell(item: item)
            }
            .width(170)
        }
        .contextMenu(forSelectionType: String.self) { paths in
            menu(items.filter { paths.contains($0.path) })
        } primaryAction: { paths in
            if let path = paths.first, let item = items.first(where: { $0.path == path }) {
                model.open(item, on: device)
            }
        }
    }

    @ViewBuilder private func menu(_ chosen: [PhoneFile]) -> some View {
        if chosen.count == 1, let item = chosen.first {
            Button("Open") { model.open(item, on: device) }
        }
        if !chosen.isEmpty {
            Button("Download to Downloads") { model.download(chosen, from: device) }
        }
        if chosen.count == 1, let item = chosen.first {
            Button("Rename…") {
                renameTo = item.name
                renaming = item
            }
        }
        if !chosen.isEmpty {
            Divider()
            Button("Delete…", role: .destructive) { deleting = chosen }
        }
    }

    private var folder: String { model.currentFolder(device) }

    private var parent: String {
        let up = (folder as NSString).deletingLastPathComponent
        return up.isEmpty ? "/" : up
    }

    private func icon(_ item: PhoneFile) -> String {
        if item.folder { return "folder.fill" }
        switch (item.name as NSString).pathExtension.lowercased() {
        case "jpg", "jpeg", "png", "gif", "webp", "heic": return "photo"
        case "mp4", "mkv", "mov", "webm", "3gp": return "film"
        case "mp3", "m4a", "aac", "ogg", "opus", "wav", "flac": return "music.note"
        case "pdf": return "doc.richtext"
        case "apk": return "app.badge"
        case "zip", "rar", "7z", "tar", "gz": return "doc.zipper"
        case "txt", "md", "log", "json", "xml", "csv": return "doc.text"
        default: return "doc"
        }
    }
}

/// "Phone › DCIM › Camera": click a part to go there.
private struct Breadcrumbs: View {
    let path: String
    let go: (String) -> Void

    var body: some View {
        let root = phoneStorage()
        let inStorage = path == root || path.hasPrefix(root + "/")
        let rest = inStorage ? String(path.dropFirst(root.count)) : path
        let parts = rest.split(separator: "/").map(String.init)
        HStack(spacing: 4) {
            if inStorage {
                Button("Phone") { go(root) }.buttonStyle(.plain).fontWeight(.semibold)
            }
            ForEach(Array(parts.enumerated()), id: \.offset) { index, part in
                if inStorage || index > 0 {
                    Image(systemName: "chevron.compact.right").foregroundStyle(.secondary)
                }
                Button(part) {
                    let prefix = inStorage ? root : ""
                    go(prefix + "/" + parts[...index].joined(separator: "/"))
                }
                .buttonStyle(.plain)
                .fontWeight(index == parts.count - 1 ? .semibold : .regular)
            }
        }
        .lineLimit(1)
    }
}

private struct SizeCell: View {
    let item: PhoneFile

    var body: some View {
        Text(item.folder ? "—" : item.size.formatted(.byteCount(style: .file)))
            .foregroundStyle(.secondary)
    }
}

private struct DateCell: View {
    let item: PhoneFile

    var body: some View {
        Text(Date(timeIntervalSince1970: TimeInterval(item.modified)), format: .dateTime.year().month().day().hour().minute())
            .foregroundStyle(.secondary)
    }
}

extension PhoneFile: Identifiable {
    public var id: String { path }
}
