/// OrbMacCore
///
/// Non-UI logic for the ADHD Focus Orb macOS app lives here:
///   - protocol client (talking to relay-rs / relay-py)
///   - audio capture/playback
///   - the focus-session state machine
///
/// This target has no import of SwiftUI/AppKit and must stay independently
/// testable (see OrbMacCoreTests). Parallel units (protocol client, audio,
/// orchestration) each add their own file(s) here rather than reaching into
/// the OrbMac app target.
///
/// Placeholder for M1 (scaffold). Real logic lands in later units.
public enum OrbMacCore {
    /// Package identity marker, useful for a trivial smoke test that the
    /// library actually built and links.
    public static let moduleName = "OrbMacCore"
}
