import Foundation

// Build the XML before sign_update signs the complete feed. No edits follow signing.
let args = CommandLine.arguments
guard args.count == 7 else {
    FileHandle.standardError.write(Data("Usage: build-appcast.swift MANIFEST NOTES EXISTING_FEED OUTPUT SIGNATURE DOWNLOAD_URL\n".utf8))
    exit(2)
}
let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: args[1]))) as! [String: Any]
func value(_ key: String) -> String { manifest[key] as! String }
let version = value("releaseVersion")
let build = value("bundleVersion")
let isBeta = version.split(separator: "+", maxSplits: 1)[0].contains("-")
let notes = try String(contentsOfFile: args[2], encoding: .utf8)
let namespace = "http://www.andymatuschak.org/xml-namespaces/sparkle"
let rss = XMLElement(name: "rss")
rss.addAttribute(XMLNode.attribute(withName: "version", stringValue: "2.0") as! XMLNode)
rss.addNamespace(XMLNode.namespace(withName: "sparkle", stringValue: namespace) as! XMLNode)
let channel = XMLElement(name: "channel")
rss.addChild(channel)
channel.addChild(XMLElement(name: "title", stringValue: "Top updates"))
channel.addChild(XMLElement(name: "link", stringValue: "https://github.com/bshk-app/top"))
channel.addChild(XMLElement(name: "language", stringValue: "en"))

var items: [XMLElement] = []
if FileManager.default.fileExists(atPath: args[3]) {
    let existing = try XMLDocument(contentsOf: URL(fileURLWithPath: args[3]))
    items = try existing.nodes(forXPath: "/rss/channel/item").map { $0.copy() as! XMLElement }
}
func itemValue(_ item: XMLElement, _ name: String) -> String {
    item.elements(forName: name).first?.stringValue ?? ""
}
let existingVersion = items.first { itemValue($0, "sparkle:shortVersionString") == version }
let highest = items.compactMap { Int(itemValue($0, "sparkle:version")) }.max() ?? 0
guard existingVersion != nil || (Int(build) ?? 0) > highest else {
    FileHandle.standardError.write(Data("Build number must exceed the published feed; release from full history or set BUILD_NUMBER.\n".utf8))
    exit(1)
}
items.removeAll { itemValue($0, "sparkle:shortVersionString") == version }
let item = XMLElement(name: "item")
item.addChild(XMLElement(name: "title", stringValue: "Top \(version)"))
item.addChild(XMLElement(name: "sparkle:version", stringValue: build))
item.addChild(XMLElement(name: "sparkle:shortVersionString", stringValue: version))
item.addChild(XMLElement(name: "sparkle:minimumSystemVersion", stringValue: value("minMacOS")))
item.addChild(XMLElement(name: "sparkle:hardwareRequirements", stringValue: "arm64"))
if isBeta { item.addChild(XMLElement(name: "sparkle:channel", stringValue: "beta")) }
let description = XMLElement(name: "description", stringValue: notes)
description.addAttribute(XMLNode.attribute(withName: "sparkle:format", stringValue: "markdown") as! XMLNode)
item.addChild(description)
let date = DateFormatter()
date.locale = Locale(identifier: "en_US_POSIX")
date.timeZone = TimeZone(secondsFromGMT: 0)
date.dateFormat = "EEE, dd MMM yyyy HH:mm:ss Z"
item.addChild(XMLElement(name: "pubDate", stringValue: date.string(from: Date())))
let enclosure = try XMLElement(xmlString: "<enclosure xmlns:sparkle=\"\(namespace)\" \(args[5])/>")
enclosure.addAttribute(XMLNode.attribute(withName: "url", stringValue: args[6]) as! XMLNode)
enclosure.addAttribute(XMLNode.attribute(withName: "type", stringValue: "application/octet-stream") as! XMLNode)
item.addChild(enclosure)
items.append(item)
items.sort { (Int(itemValue($0, "sparkle:version")) ?? 0) > (Int(itemValue($1, "sparkle:version")) ?? 0) }
var kept = Array(items.prefix(10))
if let stable = items.first(where: { itemValue($0, "sparkle:channel").isEmpty }), !kept.contains(where: { $0 === stable }) {
    kept.append(stable)
}
for entry in kept { channel.addChild(entry) }
let document = XMLDocument(rootElement: rss)
document.characterEncoding = "UTF-8"
document.version = "1.0"
try document.xmlData(options: .nodePrettyPrint).write(to: URL(fileURLWithPath: args[4]))

// Homebrew reads only stable entries, while the app's beta channel can see both.
if !isBeta {
    for entry in channel.elements(forName: "item") where !itemValue(entry, "sparkle:channel").isEmpty { entry.detach() }
    let stableOutput = URL(fileURLWithPath: args[4]).deletingLastPathComponent().appendingPathComponent("appcast-stable.xml")
    try document.xmlData(options: .nodePrettyPrint).write(to: stableOutput)
}
