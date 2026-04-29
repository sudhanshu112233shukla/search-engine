#!/usr/bin/env node
/**
 * Convert exported shard ZIPs into secure pack assets suitable for hosting on GitHub Releases.
 *
 * Inputs:
 *  - A directory containing `manifest.json` and `<lang>/shard_XXXX.zip` files (from `cargo run -- export-packs`)
 *  - A device public key (base64 X.509 DER) for device-binding
 *  - An Ed25519 signing private key PEM
 *
 * Output:
 *  - An output directory with:
 *      - manifest.json (version bumped to 2, download_base set)
 *      - <lang>_shard_XXXX.spack
 *      - <lang>_shard_XXXX.spack.sig
 *
 * Note: Because packs are device-bound, this output is intended for a specific device/user.
 */

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

function argValue(flag) {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return null;
  return process.argv[idx + 1] || null;
}

function usage() {
  console.error(
    [
      "Usage:",
      "  node scripts/secure_pack/export_secure_release.js \\",
      "    --in <exported_packs_dir> \\",
      "    --out <out_dir> \\",
      "    --lang en \\",
      "    --download-base https://github.com/<owner>/<repo>/releases/download/<tag> \\",
      '    --device-pubkey-b64 "<base64-x509-der-from-app>" \\',
      "    --signing-private-key signing_ed25519_private.pem",
    ].join("\n")
  );
  process.exit(1);
}

const inDir = argValue("--in");
const outDir = argValue("--out");
const lang = argValue("--lang") || "en";
const downloadBase = argValue("--download-base");
const devicePubKeyB64 = argValue("--device-pubkey-b64");
const signingPrivKey = argValue("--signing-private-key");

if (!inDir || !outDir || !downloadBase || !devicePubKeyB64 || !signingPrivKey) usage();

const inAbs = path.resolve(inDir);
const outAbs = path.resolve(outDir);
fs.mkdirSync(outAbs, { recursive: true });

const manifestPath = path.join(inAbs, "manifest.json");
if (!fs.existsSync(manifestPath)) {
  console.error("Missing manifest.json in:", inAbs);
  process.exit(2);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
manifest.version = Math.max(2, manifest.version || 0);
manifest.download_base = downloadBase;

fs.writeFileSync(path.join(outAbs, "manifest.json"), JSON.stringify(manifest, null, 2));

const langDir = path.join(inAbs, lang);
if (!fs.existsSync(langDir) || !fs.statSync(langDir).isDirectory()) {
  console.error("Missing language dir:", langDir);
  process.exit(2);
}

const shardZips = fs
  .readdirSync(langDir)
  .filter((n) => /^shard_\d+\.zip$/i.test(n))
  .sort();

if (shardZips.length === 0) {
  console.error("No shard_*.zip found in:", langDir);
  process.exit(2);
}

for (const zipName of shardZips) {
  const shard = zipName.replace(/\.zip$/i, "");
  const inZip = path.join(langDir, zipName);
  const outSpack = path.join(outAbs, `${lang}_${shard}.spack`);

  const res = spawnSync(
    process.execPath,
    [
      path.join(__dirname, "encrypt_spack.js"),
      "--in",
      inZip,
      "--out",
      outSpack,
      "--device-pubkey-b64",
      devicePubKeyB64,
      "--signing-private-key",
      signingPrivKey,
    ],
    { stdio: "inherit" }
  );
  if (res.status !== 0) process.exit(res.status || 3);
}

console.log("Wrote secure release assets to:", outAbs);

