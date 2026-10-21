// ocr.swift <png...> : prints recognized text lines with normalized x,y (top-left origin)
import Vision
import AppKit
for path in CommandLine.arguments.dropFirst() {
    guard let img = NSImage(contentsOfFile: path),
          let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { continue }
    let req = VNRecognizeTextRequest()
    req.recognitionLevel = .accurate
    req.usesLanguageCorrection = false
    try? VNImageRequestHandler(cgImage: cg).perform([req])
    print("== \(path)")
    for o in (req.results ?? []) {
        let b = o.boundingBox
        if let t = o.topCandidates(1).first?.string {
            print(String(format: "%.3f %.3f  %@", b.minX, 1 - b.maxY, t))
        }
    }
}
