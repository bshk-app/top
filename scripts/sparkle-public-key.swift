import CryptoKit
import Foundation

let path = CommandLine.arguments[1]
let encoded = try String(contentsOfFile: path, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
guard let seed = Data(base64Encoded: encoded), seed.count == 32 else {
    FileHandle.standardError.write(Data("Expected a base64-encoded 32-byte Sparkle seed.\n".utf8))
    exit(1)
}
let key = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
print(key.publicKey.rawRepresentation.base64EncodedString())
