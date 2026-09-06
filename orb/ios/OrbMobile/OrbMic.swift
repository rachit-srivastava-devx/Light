import AVFoundation
import React

/// Native mic capture — mirrors android/app/src/main/java/com/orbmobile/OrbMicModule.kt's
/// contract exactly (same event name/shape) so apps/mobile/src/voice/NativeMicPort.ts works
/// unchanged on both platforms: 16kHz mono PCM16, ~20ms/640-byte frames, `OrbMicFrame` events
/// shaped `{audioBase64, atMs, byteLength}`.
@objc(OrbMic)
class OrbMic: RCTEventEmitter {
  /**
   * Built fresh on each `start()`, never at init. `AVAudioEngine.inputNode` snapshots the hardware
   * input format the first time it is touched, and React Native constructs this module at bridge
   * init — long before any AVAudioSession category exists. An engine kept as a stored property
   * therefore caches a 0ch/0Hz "no input route" format and keeps reporting it even after a real
   * route appears, which is indistinguishable from genuinely having no microphone.
   */
  private var engine: AVAudioEngine?
  private var frameCount = 0
  private var startedAtMs: Double = 0

  override static func requiresMainQueueSetup() -> Bool { false }
  override func supportedEvents() -> [String]! { ["OrbMicFrame"] }

  @objc(requestPermission:rejecter:)
  func requestPermission(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    if #available(iOS 17.0, *) {
      AVAudioApplication.requestRecordPermission { granted in resolve(granted) }
    } else {
      AVAudioSession.sharedInstance().requestRecordPermission { granted in resolve(granted) }
    }
  }

  @objc(start:rejecter:)
  func start(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    do {
      let session = AVAudioSession.sharedInstance()
      // .mixWithOthers: the presence bed (react-native-audio-api) runs its own AVAudioEngine on
      // this same process-wide AVAudioSession for the .playback category. Switching the shared
      // session to .playAndRecord without this option forces a hard route reconfiguration that
      // tears the other engine's already-connected render graph out from under it — the resulting
      // internal state mismatch is what threw the uncaught NSException in engine.start() below.
      // The category is deliberately NOT set here. react-native-audio-api owns this process-wide
      // session and re-asserts its own category on every engine start / route change, so anything
      // set here is transient and gets stomped — that fight is what left the input route half-dead
      // (`channels=2 rate=0`). `playAndRecord` is instead declared once, up front, via
      // `AudioManager.setAudioSessionOptions` in App.tsx, which is the setting that actually sticks.
      // Setting a preferred rate is still safe and useful: it pins the input clock this engine gets.
      try? session.setPreferredSampleRate(48_000)
      try? session.setPreferredIOBufferDuration(0.02)
      try session.setActive(true)

      // A fresh engine every start (see the `engine` property docs): the input format is snapshotted
      // on first `inputNode` access, so reusing an engine built before the session existed pins a
      // stale 0ch/0Hz format forever.
      let engine = AVAudioEngine()
      self.engine = engine
      let input = engine.inputNode
      // `inputFormat(forBus:)` is the *hardware* input format; `outputFormat(forBus:)` is what the
      // node feeds the graph and stays uninitialized (sampleRate 0) until the engine actually runs.
      // Reading the latter is what produced the bogus `rate=0` that then got forced into
      // `installTap` and threw the uncatchable 'Input HW format is invalid'.
      var inputFormat = input.inputFormat(forBus: 0)
      NSLog(
        "OrbMic: input_route hw_channels=%u hw_rate=%.0f out_rate=%.0f session_rate=%.0f inputAvailable=%@ inputs=%ld",
        inputFormat.channelCount,
        inputFormat.sampleRate,
        input.outputFormat(forBus: 0).sampleRate,
        session.sampleRate,
        session.isInputAvailable ? "yes" : "no",
        session.availableInputs?.count ?? -1
      )

      // A 0ch/0Hz format means no input route is established *yet*. On the Simulator this is the
      // Target format mirrors OrbMicModule.kt: 16kHz mono 16-bit PCM.
      guard let targetFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true) else {
        reject("orb_mic_format_error", "could not build the 16kHz mono PCM16 target format", nil)
        return
      }
      // An invalid hardware format cannot be substituted for: `installTap` validates the format
      // against the node's own and throws an *uncatchable* ObjC NSException ('Input HW format is
      // invalid') on any mismatch — Swift `do/catch` cannot intercept it, so a wrong guess here
      // takes the whole app down (live-observed). Fail cleanly instead, reporting the real numbers.
      if inputFormat.channelCount == 0 || inputFormat.sampleRate == 0 {
        NSLog("OrbMic: unusable_input_format channels=%u rate=%.0f", inputFormat.channelCount, inputFormat.sampleRate)
        reject(
          "orb_mic_no_input_route",
          "microphone input is not usable (hardware format channels=\(inputFormat.channelCount) rate=\(inputFormat.sampleRate)). On the iOS Simulator this usually means macOS has not granted the Simulator microphone access — check System Settings > Privacy & Security > Microphone — or the Mac has no active input device (System Settings > Sound > Input).",
          nil
        )
        return
      }
      frameCount = 0
      startedAtMs = Date().timeIntervalSince1970 * 1000

      // ~20ms at 16kHz = 320 samples = 640 bytes, matching the Android module's chunk size.
      let frameCapacity: AVAudioFrameCount = 320
      // Built from the FIRST delivered buffer's own format rather than any format queried up front:
      // with `format: nil` below, AVAudioEngine picks the node's format itself, and that is the only
      // authoritative description of what actually arrives. Rebuilt if the format ever changes
      // (route switch mid-session), so the converter can never silently decode against a stale one.
      var converter: AVAudioConverter?
      var converterSourceFormat: AVAudioFormat?
      // AVAudioEngine tap callbacks are not guaranteed to be 20ms. The old implementation
      // converted each callback into a single 320-sample frame, silently discarding any converted
      // samples beyond that buffer. With a 24kHz simulator input and a 1024-frame tap this meant
      // roughly one 20ms frame was emitted every 43ms (and the bridge often delivered them ~100ms
      // apart), so STT received speech with large holes in it. Accumulate converted samples and
      // emit every complete 20ms frame instead.
      var pendingSamples = [Int16]()

      // `format: nil` makes AVAudioEngine use the node's own format, so a mismatch — the thing that
      // throws the uncatchable ObjC 'Input HW format is invalid' NSException that Swift's do/catch
      // cannot intercept — is structurally impossible here.
      input.installTap(onBus: 0, bufferSize: 1024, format: nil) { [weak self] buffer, _ in
        guard let self else { return }
        if converterSourceFormat != buffer.format {
          converter = AVAudioConverter(from: buffer.format, to: targetFormat)
          converterSourceFormat = buffer.format
          NSLog("OrbMic: converter_ready from_rate=%.0f from_channels=%u", buffer.format.sampleRate, buffer.format.channelCount)
        }
        guard let converter else { return }
        let targetCapacity = AVAudioFrameCount(
          ceil(Double(buffer.frameLength) * targetFormat.sampleRate / buffer.format.sampleRate)
        ) + 2
        guard let outBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: targetCapacity) else { return }
        var error: NSError?
        var suppliedInput = false
        let inputBlock: AVAudioConverterInputBlock = { _, outStatus in
          guard !suppliedInput else {
            outStatus.pointee = .noDataNow
            return nil
          }
          suppliedInput = true
          outStatus.pointee = .haveData
          return buffer
        }
        converter.convert(to: outBuffer, error: &error, withInputFrom: inputBlock)
        if error != nil || outBuffer.frameLength == 0 { return }
        guard let channelData = outBuffer.int16ChannelData else { return }
        let convertedSamples = Int(outBuffer.frameLength)
        pendingSamples.append(contentsOf: UnsafeBufferPointer(start: channelData[0], count: convertedSamples))

        let samplesPerFrame = Int(frameCapacity)
        while pendingSamples.count >= samplesPerFrame {
          let frameSamples = Array(pendingSamples.prefix(samplesPerFrame))
          pendingSamples.removeFirst(samplesPerFrame)
          let data = frameSamples.withUnsafeBytes { Data($0) }
          self.frameCount += 1
          let atMs = self.startedAtMs + Double(self.frameCount) * 20.0
          if self.frameCount % 50 == 0 {
            NSLog("OrbMic: frames_captured count=%d last_bytes=%d pending_samples=%d", self.frameCount, data.count, pendingSamples.count)
          }
          self.sendEvent(withName: "OrbMicFrame", body: [
            "audioBase64": data.base64EncodedString(),
            "atMs": atMs,
            "byteLength": data.count,
          ])
        }
      }

      // `setActive(true)` above posts an AVAudioEngineConfigurationChange notification that the
      // presence bed's own AVAudioEngine (react-native-audio-api) handles by rebuilding its own
      // render graph, on the main queue. Calling `engine.start()` synchronously in the same call
      // stack races that rebuild for the same hardware I/O — live-observed as an uncatchable
      // AURemoteIO RPC-timeout abort while the presence bed's rebuild was still in flight on the
      // main thread. Deferring to the next main-queue turn lets that rebuild finish first (it was
      // already queued by the time `setActive` returned, so FIFO ordering puts it ahead of this).
      DispatchQueue.main.async {
        do {
          if !engine.isRunning {
            engine.prepare()
            try engine.start()
          }
          NSLog("OrbMic: start_recording")
          resolve(nil)
        } catch {
          // On Simulator this is the expected outcome of the very first attempt when macOS has not
          // yet granted the Simulator microphone access: the open fails, but the attempt is what
          // makes macOS show its prompt / list the Simulator under Privacy > Microphone at all.
          // Granting it and relaunching then succeeds.
          NSLog("OrbMic: start_failed %@", error.localizedDescription)
          reject("orb_mic_start_error", error.localizedDescription, error)
        }
      }
    } catch {
      reject("orb_mic_start_error", error.localizedDescription, error)
    }
  }

  @objc(stop:rejecter:)
  func stop(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    engine?.inputNode.removeTap(onBus: 0)
    engine?.stop()
    engine = nil
    NSLog("OrbMic: stop_recording")
    try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    resolve(nil)
  }
}
