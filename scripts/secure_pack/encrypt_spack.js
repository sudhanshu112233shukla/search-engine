#!/usr/bin/env node
/**
 * Encrypt a shard zip into a secure pack (`.spack`) and produce a detached Ed25519 signature (`.spack.sig`).
 *
 * Format:
 *   magic "SPK1" (4 bytes)
 *   version (1 byte) = 1
 *   nonce_len (1 byte) = 12
 *   nonce (12 bytes)
 *   wrapped_dek_len (2 bytes, big-endian)
 *   wrapped_dek (RSA-OAEP-SHA256 encrypted 32-byte DEK, using device public key)
 *   ciphertext (AES-256-GCM over the input zip bytes, AAD = header bytes)
 *
 * The `.spack.sig` file contains base64(signature) where signature is Ed25519 over the entire `.spack` bytes.
 */

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

function argValue(flag) {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return null;
  return process.argv[idx + 1] || null;
}

function usage() {
  console.error(
    [
      "Usage:",
      '  node scripts/secure_pack/encrypt_spack.js --in shard_0000.zip --out shard_0000.spack \\',
      "    --device-pubkey-b64 <base64-x509-der> --signing-private-key <ed25519-priv.pem>",
      "",
      "Notes:",
      "- `--device-pubkey-b64` is the base64 X.509 DER public key shown in the Android app settings.",
      "- `--signing-private-key` is an Ed25519 private key in PEM (keep secret).",
    ].join("\n")
  );
  process.exit(1);
}

const inPath = argValue("--in");
const outPath = argValue("--out");
const devicePubKeyB64 = argValue("--device-pubkey-b64");
const signingPrivKeyPath = argValue("--signing-private-key");

if (!inPath || !outPath || !devicePubKeyB64 || !signingPrivKeyPath) usage();

const inAbs = path.resolve(inPath);
const outAbs = path.resolve(outPath);
const sigAbs = `${outAbs}.sig`;

const deviceDer = Buffer.from(devicePubKeyB64, "base64");
const devicePubKey = crypto.createPublicKey({ key: deviceDer, format: "der", type: "spki" });

const signingPrivPem = fs.readFileSync(signingPrivKeyPath, "utf8");
const signingPrivKey = crypto.createPrivateKey(signingPrivPem);

const dek = crypto.randomBytes(32);
const nonce = crypto.randomBytes(12);

const wrappedDek = crypto.publicEncrypt(
  {
    key: devicePubKey,
    oaepHash: "sha256",
    padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
  },
  dek
);

const header = Buffer.concat([
  Buffer.from("SPK1", "ascii"),
  Buffer.from([1]),
  Buffer.from([nonce.length]),
  nonce,
  Buffer.from([(wrappedDek.length >> 8) & 0xff, wrappedDek.length & 0xff]),
  wrappedDek,
]);

const cipher = crypto.createCipheriv("aes-256-gcm", dek, nonce);
cipher.setAAD(header);

const hash = crypto.createHash("sha256");
const out = fs.createWriteStream(outAbs);
out.write(header);
hash.update(header);

const inStream = fs.createReadStream(inAbs);
inStream.on("error", (err) => {
  console.error("Read error:", err.message);
  process.exit(2);
});

out.on("error", (err) => {
  console.error("Write error:", err.message);
  process.exit(2);
});

function writeAndHash(buf) {
  out.write(buf);
  hash.update(buf);
}

inStream.on("data", (chunk) => {
  const enc = cipher.update(chunk);
  if (enc.length) writeAndHash(enc);
});

inStream.on("end", () => {
  const final = cipher.final();
  if (final.length) writeAndHash(final);
  const tag = cipher.getAuthTag();
  writeAndHash(tag);
  out.end();

  out.on("close", () => {
    // Sign the final file bytes by re-reading it (streaming sign is ok but easiest is to sign hash here).
    // We sign the full bytes by signing the SHA-256 digest (as an application convention).
    const digest = hash.digest();
    const signature = crypto.sign(null, digest, signingPrivKey);
    fs.writeFileSync(sigAbs, signature.toString("base64"));
    console.log("Wrote:", outAbs);
    console.log("Wrote:", sigAbs);
    console.log("Digest (sha256):", digest.toString("hex"));
  });
});

