import SwiftUI

// Notification names used to bridge menu-bar commands → PlayerView
extension Notification.Name {
    static let magentaLoadModel    = Notification.Name("MagentaLoadModel")
    static let magentaTogglePlay   = Notification.Name("MagentaTogglePlay")   // smart toggle
    static let magentaStop         = Notification.Name("MagentaStop")
    static let magentaResetContext = Notification.Name("MagentaResetContext")
    static let magentaShowAbout    = Notification.Name("MagentaShowAbout")
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
            // Standard About panel replaced with our custom sheet
            CommandGroup(replacing: .appInfo) {
                Button("About Magenta Player") {
                    NotificationCenter.default.post(name: .magentaShowAbout, object: nil)
                }
            }

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
                Button("Play / Pause / Resume") {
                    NotificationCenter.default.post(name: .magentaTogglePlay, object: nil)
                }
                .keyboardShortcut(" ", modifiers: [])

                Button("Stop") {
                    NotificationCenter.default.post(name: .magentaStop, object: nil)
                }
                .keyboardShortcut(".", modifiers: .command)

                Divider()

                Button("Reset Context") {
                    NotificationCenter.default.post(name: .magentaResetContext, object: nil)
                }
                .keyboardShortcut("r", modifiers: .command)
            }
        }
    }
}
