package com.app.search

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.io.BufferedInputStream
import java.io.DataInputStream
import java.io.File
import java.io.InputStream
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.PublicKey
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.CipherInputStream
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import java.util.zip.ZipInputStream
import org.bouncycastle.asn1.ASN1Primitive
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer

object SecurePackCrypto {
    private const val DEVICE_WRAP_KEY_ALIAS = "pack_wrap_key_rsa_oaep_sha256_v1"

    private const val PACK_MAGIC = "SPK1"
    private const val PACK_VERSION: Int = 1

    // Replace this with your real Ed25519 public key in SPKI DER form (base64-encoded).
    // This key is used to verify pack signatures.
    private const val TRUSTED_PACK_PUBKEY_SPKI_DER_B64 = "AAAA"

    fun ensureDeviceWrapKeyPair() {
        val ks = KeyStore.getInstance("AndroidKeyStore")
        ks.load(null)
        if (ks.containsAlias(DEVICE_WRAP_KEY_ALIAS)) return

        val kpg = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, "AndroidKeyStore")
        kpg.initialize(
            KeyGenParameterSpec.Builder(
                DEVICE_WRAP_KEY_ALIAS,
                KeyProperties.PURPOSE_DECRYPT
            )
                .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA512)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
                .setKeySize(2048)
                .build()
        )
        kpg.generateKeyPair()
    }

    fun devicePublicKeyBase64(): String {
        val ks = KeyStore.getInstance("AndroidKeyStore")
        ks.load(null)
        val cert = ks.getCertificate(DEVICE_WRAP_KEY_ALIAS) ?: return ""
        val pub = cert.publicKey.encoded ?: return ""
        return Base64.encodeToString(pub, Base64.NO_WRAP)
    }

    private fun loadDevicePrivateKey(): PrivateKey? {
        val ks = KeyStore.getInstance("AndroidKeyStore")
        ks.load(null)
        return ks.getKey(DEVICE_WRAP_KEY_ALIAS, null) as? PrivateKey
    }

    private fun unwrapDekWithDeviceKey(wrappedDek: ByteArray): ByteArray? {
        val priv = loadDevicePrivateKey() ?: return null
        val cipher = Cipher.getInstance("RSA/ECB/OAEPWithSHA-256AndMGF1Padding")
        cipher.init(Cipher.DECRYPT_MODE, priv)
        return runCatching { cipher.doFinal(wrappedDek) }.getOrNull()
    }

    private data class Header(
        val version: Int,
        val nonce: ByteArray,
        val wrappedDek: ByteArray,
        val headerBytes: ByteArray
    )

    private fun parseHeader(input: InputStream): Header? {
        val buffered = if (input is BufferedInputStream) input else BufferedInputStream(input, 64 * 1024)
        buffered.mark(64 * 1024)
        val dis = DataInputStream(buffered)
        val magicBytes = ByteArray(4)
        dis.readFully(magicBytes)
        val magic = String(magicBytes, Charsets.US_ASCII)
        if (magic != PACK_MAGIC) return null
        val version = dis.readUnsignedByte()
        if (version != PACK_VERSION) return null
        val nonceLen = dis.readUnsignedByte()
        if (nonceLen !in 12..32) return null
        val nonce = ByteArray(nonceLen)
        dis.readFully(nonce)
        val wrappedLen = dis.readUnsignedShort()
        if (wrappedLen !in 32..4096) return null
        val wrapped = ByteArray(wrappedLen)
        dis.readFully(wrapped)

        buffered.reset()
        val headerSize = 4 + 1 + 1 + nonceLen + 2 + wrappedLen
        val headerBytes = ByteArray(headerSize)
        dis.readFully(headerBytes)
        return Header(version = version, nonce = nonce, wrappedDek = wrapped, headerBytes = headerBytes)
    }

    fun verifyTrustedSignature(packFile: File, sigB64: String): Boolean {
        val pubDer = runCatching { Base64.decode(TRUSTED_PACK_PUBKEY_SPKI_DER_B64, Base64.DEFAULT) }.getOrNull() ?: return false
        val pubKeyRaw = runCatching {
            val info = SubjectPublicKeyInfo.getInstance(ASN1Primitive.fromByteArray(pubDer))
            info.publicKeyData.octets
        }.getOrNull() ?: return false
        if (pubKeyRaw.size != 32) return false
        val sig = runCatching { Base64.decode(sigB64.trim(), Base64.DEFAULT) }.getOrNull() ?: return false
        if (sig.size != 64) return false

        // Signature is over SHA-256(pack_bytes) to allow streaming verification for very large packs.
        val digest = MessageDigest.getInstance("SHA-256")
        BufferedInputStream(packFile.inputStream(), 1024 * 1024).use { input ->
            val buf = ByteArray(1024 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n <= 0) break
                digest.update(buf, 0, n)
            }
        }
        val hash = digest.digest()

        val signer = Ed25519Signer()
        signer.init(false, Ed25519PublicKeyParameters(pubKeyRaw, 0))
        signer.update(hash, 0, hash.size)
        return signer.verifySignature(sig)
    }

    fun decryptAndUnzipToDir(context: Context, securePackFile: File, outDir: File): Boolean {
        ensureDeviceWrapKeyPair()

        val input = BufferedInputStream(securePackFile.inputStream(), 64 * 1024)
        input.use { stream ->
            val header = parseHeader(stream) ?: return false
            val dek = unwrapDekWithDeviceKey(header.wrappedDek) ?: return false
            if (dek.size != 32) return false

            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            val key = SecretKeySpec(dek, "AES")
            val gcm = GCMParameterSpec(128, header.nonce)
            cipher.init(Cipher.DECRYPT_MODE, key, gcm)
            cipher.updateAAD(header.headerBytes)

            // The stream is positioned immediately after the header bytes (parseHeader read them).
            // Wrap the remaining ciphertext stream and unzip directly (no decrypted zip written to disk).
            val cipherStream = CipherInputStream(stream, cipher)
            return unzipStream(cipherStream, outDir)
        }
    }

    private fun unzipStream(input: InputStream, outDir: File): Boolean {
        try {
            if (outDir.exists()) outDir.deleteRecursively()
            outDir.mkdirs()
            ZipInputStream(BufferedInputStream(input, 64 * 1024)).use { zis ->
                var entry = zis.nextEntry
                while (entry != null) {
                    val name = entry.name
                    if (name.contains("..") || name.startsWith("/") || name.startsWith("\\")) {
                        return false
                    }
                    val outFile = File(outDir, name)
                    if (entry.isDirectory) {
                        outFile.mkdirs()
                    } else {
                        outFile.parentFile?.mkdirs()
                        outFile.outputStream().use { os ->
                            zis.copyTo(os)
                        }
                    }
                    zis.closeEntry()
                    entry = zis.nextEntry
                }
            }
            return true
        } catch (_: Exception) {
            outDir.deleteRecursively()
            return false
        }
    }
}
