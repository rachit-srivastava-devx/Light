import SwiftUI
import OrbMacCore

/// Entry point for the ADHD Focus Orb macOS app.
///
/// This target owns UI only. Non-UI logic (protocol client, audio,
/// state machine) lives in `OrbMacCore` and is imported here.
@main
struct OrbMacApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowResizability(.contentSize)
    }
}

/// Root view: hosts the real orb visual rendering (`OrbView`, unit M-visual), replacing the
/// former placeholder colored circle. Bound to a `@State` `OrbVisualState`, defaulted to
/// `.booting` — driving this from the protocol client's real session state is a later
/// integration unit, not this one's scope.
struct ContentView: View {
    @State private var orbState: OrbVisualState = .booting

    var body: some View {
        OrbView(state: orbState)
            .padding(40)
    }
}

#Preview {
    ContentView()
}
