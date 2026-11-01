import SwiftUI

/// The phone's newest photos and videos; click one to open it on the Mac.
struct PhotosGrid: View {
    @Environment(AppModel.self) private var model
    let device: Device

    var body: some View {
        let photos = model.photos[device.serial] ?? []
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("Recent Photos").font(.title2.bold())
                Spacer()
                if model.photosLoading.contains(device.serial) {
                    ProgressView().controlSize(.small)
                }
                Button("Refresh", systemImage: "arrow.clockwise") { model.loadPhotos(device) }
                    .buttonStyle(.glass)
                    .labelStyle(.iconOnly)
            }
            Text("From the camera and screenshots. Click one to open it; right-click to download it.")
                .foregroundStyle(.secondary)
            if photos.isEmpty && !model.photosLoading.contains(device.serial) {
                ContentUnavailableView("No Photos", systemImage: "photo.on.rectangle", description: Text("Photos taken on the phone show up here."))
            } else {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 8)], spacing: 8) {
                    ForEach(photos, id: \.file.path) { photo in
                        PhotoTile(photo: photo)
                            .onTapGesture { model.open(photo.file, on: device) }
                            .contextMenu {
                                Button("Open") { model.open(photo.file, on: device) }
                                Button("Download to Downloads") { model.download([photo.file], from: device) }
                            }
                    }
                }
            }
        }
        .task(id: device.serial) {
            if model.photos[device.serial] == nil { model.loadPhotos(device) }
        }
    }
}

private struct PhotoTile: View {
    let photo: Photo

    var body: some View {
        Color.secondary.opacity(0.15)
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                if let path = photo.thumbnailPath, let image = IconCache.image(at: path) {
                    Image(nsImage: image).resizable().scaledToFill()
                } else {
                    Image(systemName: "photo").font(.largeTitle).foregroundStyle(.secondary)
                }
            }
            .overlay(alignment: .bottomTrailing) {
                if isVideo {
                    Image(systemName: "play.circle.fill")
                        .font(.title2)
                        .foregroundStyle(.white)
                        .shadow(radius: 3)
                        .padding(6)
                }
            }
            .clipShape(.rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
            .help(photo.file.name)
    }

    private var isVideo: Bool {
        ["mp4", "mkv", "webm", "3gp", "mov"].contains((photo.file.name as NSString).pathExtension.lowercased())
    }
}
