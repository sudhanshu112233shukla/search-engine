# Packs (Offline Dataset Bundles)

This project uses "packs" to ship large offline indexes to mobile devices.

A pack is a directory containing:

- `manifest.json` (metadata + shard list)
- `en/shard_XXXX/` (one or more shard directories)

Each shard directory is a self-contained on-disk index that can be loaded by the Rust engine.

## Where Packs Live

Rust build output (local machine):

- `search_engine_rust/data/packs/`

Android expects packs in one of two locations:

- Downloaded packs: `filesDir/packs_download/<lang>/<profile>/shard_XXXX/`
- Asset fallback (only for small packs): `android_app/app/src/main/assets/packs/<lang>/shard_XXXX/`

Large packs (GBs) should be installed as "downloaded packs" (not bundled in the APK).

If you need encryption/signature/device-binding for packs, see `docs/SECURE_PACKS.md`.

## Validate A Pack (Rust)

From `search_engine_rust/`:

```bash
cargo run --release -- validate-pack --dir data/packs/en --smoke-query "what is google"
```

Show pack stats:

```bash
cargo run --release -- pack-info --dir data/packs/en
```

Run an actual query against a single shard:

```bash
cargo run --release -- search-index --dir data/packs/en/shard_0000 --query "what is google"
```

## Install Pack On Android (Manual Sideload)

This is the fastest way to test large packs without implementing hosting/downloading.

1. Pick the profile directory you want to use in the app (example: `power`).
2. Copy shards into the app's `filesDir` download location.

Android path on device storage:

- `/sdcard/Android/data/com.app.search/files/packs_download/en/power/`

Example with ADB (run from your PC):

```bash
adb shell mkdir -p /sdcard/Android/data/com.app.search/files/packs_download/en/power
adb push search_engine_rust/data/packs/en/shard_0000 /sdcard/Android/data/com.app.search/files/packs_download/en/power/
adb push search_engine_rust/data/packs/en/shard_0001 /sdcard/Android/data/com.app.search/files/packs_download/en/power/
```

Then launch the app and select the matching `language/profile`.

## Hosted Downloads (Zip Shards)

The Android downloader expects:

- `manifest.json` bundled in assets at `android_app/app/src/main/assets/packs/manifest.json`
- Shard zips hosted at:
- `${download_base}/{lang}/{shard_name}.zip`

Example:

- `https://example.com/packs/en/shard_0000.zip`

Each zip should unzip into a folder named `shard_0000/` containing the shard files.

## Export Zip Shards (Rust)

From `search_engine_rust/`:

```bash
# Fastest: store (no compression)
cargo run --release -- export-packs --in data/packs --out dist/packs_demo --method stored --download-base https://example.com/packs

# Smaller (slower): deflate compression
cargo run --release -- export-packs --in data/packs --out dist/packs_demo --method deflate --download-base https://example.com/packs
```

Output:

- `dist/packs_demo/manifest.json`
- `dist/packs_demo/en/shard_0000.zip` ... `shard_XXXX.zip`

Note: for demo packs, the exporter stamps the `power` profile `max_bytes` to include all shards.

## Recommended Demo Distribution

Large packs should not be committed to git. For a public demo:

- Build a demo pack (smaller corpus) and export shard zips.
- Upload `manifest.json` and `en/shard_XXXX.zip` to a hosting location.
- Set `download_base` to that host so the Android app can download packs in-app.

For GitHub Releases specifically, keep each asset under GitHub's per-file limit.
If a shard zip is too large, reduce `--max-docs` during `pack` or build a smaller demo corpus.

Scripted demo build:

```bash
powershell -ExecutionPolicy Bypass -File scripts/build_demo_pack.ps1
```
