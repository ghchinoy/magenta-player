import SwiftUI

// Notification names used to bridge menu-bar commands → PlayerView
extension Notification.Name {
    static let magentaLoadModel  = Notification.Name("MagentaLoadModel")
    static let magentaTogglePlay = Notification.Name("MagentaTogglePlay")
}

@main
struct MagentaPlayerApp: App {
    var body: some Scene {
        WindowGroup {
            PlayerView()
                .frame(minWidth: 800, minHeight: 470)
        }
        .windowStyle(.hiddenTitleBar)
        .windowToolbarStyle(.unified(showsTitle: false))
        .commands {
            // Remove "New" — this is not a document-based app
            CommandGroup(replacing: .newItem) {}

            // File > Load Model…  (Cmd+O)
            CommandGroup(after: .newItem) {
                Button("Load Model…") {
                    NotificationCenter.default.post(name: .magentaLoadModel, object: nil)
                }
                .keyboardShortcut("o", modifiers: .command)
            }

            // Player menu
            CommandMenu("Player") {
                Button("Play / Stop") {
                    NotificationCenter.default.post(name: .magentaTogglePlay, object: nil)
                }
                .keyboardShortcut(" ", modifiers: [])
            }
        }
    }
}
