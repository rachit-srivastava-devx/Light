import AVFoundation
import React

@objc(OrbSpeech)
class OrbSpeech: NSObject {
  private let synthesizer = AVSpeechSynthesizer()

  @objc(speak:resolver:rejecter:)
  func speak(
    _ text: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty {
      resolve(nil)
      return
    }
    DispatchQueue.main.async {
      self.synthesizer.stopSpeaking(at: .immediate)
      let utterance = AVSpeechUtterance(string: trimmed)
      utterance.rate = AVSpeechUtteranceDefaultSpeechRate * 0.9
      utterance.pitchMultiplier = 1.0
      self.synthesizer.speak(utterance)
      resolve(nil)
    }
  }

  @objc(stop:rejecter:)
  func stop(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    DispatchQueue.main.async {
      self.synthesizer.stopSpeaking(at: .immediate)
      resolve(nil)
    }
  }

  @objc
  static func requiresMainQueueSetup() -> Bool {
    false
  }
}
