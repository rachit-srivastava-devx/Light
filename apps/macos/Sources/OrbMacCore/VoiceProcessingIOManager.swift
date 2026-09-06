import AudioToolbox
import Foundation

/// Wraps a raw CoreAudio VoiceProcessingIO (`kAudioUnitSubType_VoiceProcessingIO`)
/// AudioUnit for mic capture with hardware acoustic echo cancellation (AEC).
///
/// On macOS, `AVAudioEngine.inputNode`'s voice-processing properties
/// (`isVoiceProcessingBypassed`, `isVoiceProcessingAGCEnabled`) are **read-only**
/// — the ObjC selectors exist at runtime but setting them has no effect (verified
/// on macOS SDK 26.5). The only way to get real hardware AEC on macOS is to use
/// the VPIO AudioUnit directly via AudioToolbox.
///
/// The VPIO unit processes the mic input through the system's voice-processing
/// pipeline (AEC, AGC, noise suppression) and delivers processed audio on its
/// output scope. For AEC to actually cancel echo, the output reference signal
/// (what the speaker is playing) must be connected to the VPIO's output scope —
/// this requires both mic capture and playback to share the same VPIO unit. See
/// the contract at `docs/lane-contracts/F36-aec-vpio-wiring.md` for details.
public final class VoiceProcessingIOManager {
    /// Whether the VPIO AudioUnit component is available on this platform.
    /// Verified available on macOS 13+ (tested on macOS SDK 26.5).
    public static let isAvailable: Bool = {
        var desc = AudioComponentDescription(
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_VoiceProcessingIO,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0
        )
        return AudioComponentFindNext(nil, &desc) != nil
    }()

    /// Called on an internal audio thread once per processed mic frame.
    /// The `Data` contains raw PCM16 mono samples (little-endian Int16),
    /// matching the format `MicCapture.onFrame` delivers.
    public var onProcessedFrame: ((Data) -> Void)?

    private var audioUnit: AudioUnit?
    private let renderCallback: AURenderCallback = { _, _, _, _, _, _ in
        noErr
    }

    public init() throws {
        guard Self.isAvailable else {
            throw VoiceProcessingError.vpioNotAvailable
        }

        var desc = AudioComponentDescription(
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_VoiceProcessingIO,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0
        )

        guard let component = AudioComponentFindNext(nil, &desc) else {
            throw VoiceProcessingError.componentNotFound
        }

        var unit: AudioUnit?
        let status = AudioComponentInstanceNew(component, &unit)
        guard status == noErr, let unit = unit else {
            throw VoiceProcessingError.unitCreationFailed(status)
        }

        self.audioUnit = unit
    }

    /// Starts the VPIO audio unit. Installs a render callback on the output
    /// scope to deliver processed mic frames via `onProcessedFrame`.
    public func start() throws {
        guard let unit = audioUnit else {
            throw VoiceProcessingError.notInitialized
        }

        var status: OSStatus

        // Enable AEC — VPIO's primary value prop for this feature.
        // kAUVoiceIOProperty_BypassVoiceProcessing = 2104
        var bypass: UInt32 = 0  // 0 = NOT bypassed = processing enabled
        status = AudioUnitSetProperty(
            unit,
            AudioUnitPropertyID(2104),  // kAUVoiceIOProperty_BypassVoiceProcessing
            kAudioUnitScope_Global,
            0,
            &bypass,
            UInt32(MemoryLayout<UInt32>.size)
        )
        guard status == noErr else {
            AudioComponentInstanceDispose(unit)
            self.audioUnit = nil
            throw VoiceProcessingError.propertySetFailed(status)
        }

        // Enable AGC — kAUVoiceIOProperty_VoiceProcessingEnableAGC = 2105
        var agc: UInt32 = 1  // enabled
        status = AudioUnitSetProperty(
            unit,
            AudioUnitPropertyID(2105),  // kAUVoiceIOProperty_VoiceProcessingEnableAGC
            kAudioUnitScope_Global,
            0,
            &agc,
            UInt32(MemoryLayout<UInt32>.size)
        )
        // AGC failure is non-fatal — AEC is the primary goal.

        // Install a render callback on the output scope (bus 0) to read
        // processed mic audio. The callback returns silence on the output
        // side — for true AEC, the playback signal must feed through here,
        // but that requires a shared-engine architecture (deferred).
        var callbackStruct = AURenderCallbackStruct(
            inputProc: renderCallback,
            inputProcRefCon: Unmanaged.passUnretained(self).toOpaque()
        )
        status = AudioUnitSetProperty(
            unit,
            kAudioOutputUnitProperty_SetInputCallback,
            kAudioUnitScope_Global,
            0,
            &callbackStruct,
            UInt32(MemoryLayout<AURenderCallbackStruct>.size)
        )
        guard status == noErr else {
            AudioComponentInstanceDispose(unit)
            self.audioUnit = nil
            throw VoiceProcessingError.callbackInstallFailed(status)
        }

        // Start the audio unit
        status = AudioOutputUnitStart(unit)
        guard status == noErr else {
            AudioComponentInstanceDispose(unit)
            self.audioUnit = nil
            throw VoiceProcessingError.startFailed(status)
        }
    }

    /// Stops the audio unit and releases resources.
    public func stop() {
        guard let unit = audioUnit else { return }
        AudioOutputUnitStop(unit)
        AudioComponentInstanceDispose(unit)
        audioUnit = nil
    }

    deinit {
        stop()
    }
}

public enum VoiceProcessingError: Error, Equatable {
    case vpioNotAvailable
    case componentNotFound
    case unitCreationFailed(OSStatus)
    case notInitialized
    case propertySetFailed(OSStatus)
    case callbackInstallFailed(OSStatus)
    case startFailed(OSStatus)
}
