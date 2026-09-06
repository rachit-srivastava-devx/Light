import AppKit
import Foundation
import Vision

guard CommandLine.arguments.count == 2 else {
    fputs("usage: ocr.swift IMAGE\n", stderr)
    exit(2)
}

let imageURL = URL(fileURLWithPath: CommandLine.arguments[1])
guard let image = NSImage(contentsOf: imageURL),
      let tiff = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiff),
      let cgImage = bitmap.cgImage else {
    fputs("could not read image\n", stderr)
    exit(3)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = true
let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
    let text = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
    // Sample the center half of the screen, where the Orb lives, and exclude the status bar.
    // A coarse mean is intentionally stable across PNG metadata and the clock while still
    // exposing the large state-driven Orb color changes used by the harness UI check.
    let xStart = bitmap.pixelsWide / 4
    let xEnd = bitmap.pixelsWide * 3 / 4
    let yStart = bitmap.pixelsHigh / 4
    let yEnd = bitmap.pixelsHigh * 3 / 4
    let stride = max(1, min(bitmap.pixelsWide, bitmap.pixelsHigh) / 160)
    var red = 0.0
    var green = 0.0
    var blue = 0.0
    var samples = 0.0
    for y in Swift.stride(from: yStart, to: yEnd, by: stride) {
        for x in Swift.stride(from: xStart, to: xEnd, by: stride) {
            guard let color = bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
            red += color.redComponent * 255.0
            green += color.greenComponent * 255.0
            blue += color.blueComponent * 255.0
            samples += 1.0
        }
    }
    let centerRGB: [Double] = samples == 0
        ? [0.0, 0.0, 0.0]
        : [red / samples, green / samples, blue / samples]
    let payload: [String: Any] = [
        "image": imageURL.path,
        "text": text,
        "center_rgb": centerRGB,
        "center_samples": Int(samples),
    ]
    let data = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
} catch {
    fputs("OCR failed: \(error)\n", stderr)
    exit(4)
}
