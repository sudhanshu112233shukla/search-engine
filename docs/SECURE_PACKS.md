# Secure Packs (Encrypted + Signed + Device-Bound)

This repo supports a "secure pack" (`.spack`) format intended for distributing large offline indexes (multi‑GB) while:

- encrypting data at rest (AES‑256‑GCM),
- verifying authenticity/integrity (Ed25519 signature),
- binding packs to a specific device (RSA‑OAEP wrapped data key; private key lives in Android Keystore),
- staying fully offline after one-time activation (device key generation + pack download/install).

Important: This **does not** make exfiltration impossible against a determined attacker (the device must be able to decrypt to search). It *does* prevent casual copying/sharing of packs and ensures only trusted packs load.

## Android Overview

- The app creates a **non‑exportable** RSA private key in Android Keystore on first run.
- The app exposes the **public key** (base64 X.509 DER) in Settings → "Device activation".
- Packs are encrypted with a random 32‑byte data key (DEK). The DEK is wrapped with the device public key (RSA‑OAEP‑SHA256) and embedded in the `.spack` header.
- The app downloads:
  - `${download_base}/{lang}/{shard}.spack`
  - `${download_base}/{lang}/{shard}.spack.sig`
- The app verifies the signature and then decrypts and unzips directly into the pack directory before calling JNI.

## `.spack` File Format

```
magic "SPK1" (4 bytes)
version (1 byte) = 1
nonce_len (1 byte) = 12
nonce (12 bytes)
wrapped_dek_len (2 bytes, big-endian)
wrapped_dek (RSA-OAEP-SHA256(wrap(DEK)))
ciphertext (AES-256-GCM(zip_bytes, AAD=header_bytes))
gcm_tag (16 bytes; appended by the encryptor)
```

Signature (`.spack.sig`) is:
- `base64( Ed25519( SHA-256(spack_bytes) ) )`

## Manifest Changes

To enable secure downloads, set `manifest.json` `version` to `2` (or higher). The app uses secure downloads when `version >= 2`.

## Example Workflow

### 1) On Android (one-time activation)

- Open the app → Settings → copy the **Device public key**.

### 2) On your build machine (pack builder)

1. Export shard zip(s) as usual (existing flow in `docs/PACKS.md`).
2. Encrypt each shard zip to `.spack` using the device public key:

```bash
node scripts/secure_pack/encrypt_spack.js \
  --in shard_0000.zip \
  --out shard_0000.spack \
  --device-pubkey-b64 "<base64-x509-der-from-app>" \
  --signing-private-key signing_ed25519_private.pem
```

3. Upload:
   - `shard_0000.spack`
   - `shard_0000.spack.sig`

### 3) In the app

- Tap "Download pack" (Settings). After download + verify + decrypt, the engine loads the decrypted shard directory from internal storage.

## Trusted Signing Key

The Ed25519 verification public key is embedded in:

- `android_app/app/src/main/java/com/app/search/SecurePackCrypto.kt`

Replace `TRUSTED_PACK_PUBKEY_SPKI_DER_B64` with your real public key (SPKI DER, base64).

One way to generate an Ed25519 keypair (OpenSSL):

```bash
openssl genpkey -algorithm Ed25519 -out signing_ed25519_private.pem
openssl pkey -in signing_ed25519_private.pem -pubout -out signing_ed25519_public.pem
```

To get base64 SPKI DER for `TRUSTED_PACK_PUBKEY_SPKI_DER_B64`:

```bash
node -e "const fs=require('fs');const crypto=require('crypto');const k=crypto.createPublicKey(fs.readFileSync('signing_ed25519_public.pem'));const der=k.export({format:'der',type:'spki'});console.log(der.toString('base64'));"
```
