package com.srsd.gallery;

import javax.crypto.Cipher;
import javax.crypto.SecretKey;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;
import java.security.GeneralSecurityException;
import java.security.SecureRandom;
import java.security.spec.KeySpec;

/**
 * Key derivation (PBKDF2) and AES-256-GCM encrypt/decrypt for log records.
 */
final class CryptoService {
    private static final String PBKDF2 = "PBKDF2WithHmacSHA256";
    private static final String AES_GCM = "AES/GCM/NoPadding";
    private final SecretKey aesKey;

    CryptoService(String token, byte[] salt) throws GeneralSecurityException {
        SecretKeyFactory skf = SecretKeyFactory.getInstance(PBKDF2);
        KeySpec spec = new PBEKeySpec(
                token.toCharArray(),
                salt,
                Constants.PBKDF2_ITERATIONS,
                Constants.AES_KEY_BITS);
        byte[] raw = skf.generateSecret(spec).getEncoded();
        this.aesKey = new SecretKeySpec(raw, "AES");
    }

    byte[] encrypt(byte[] plaintext, byte[] iv) throws GeneralSecurityException {
        Cipher c = Cipher.getInstance(AES_GCM);
        GCMParameterSpec gcm = new GCMParameterSpec(Constants.GCM_TAG_BITS, iv);
        c.init(Cipher.ENCRYPT_MODE, aesKey, gcm);
        return c.doFinal(plaintext);
    }

    byte[] decrypt(byte[] ciphertext, byte[] iv) throws GeneralSecurityException {
        Cipher c = Cipher.getInstance(AES_GCM);
        GCMParameterSpec gcm = new GCMParameterSpec(Constants.GCM_TAG_BITS, iv);
        c.init(Cipher.DECRYPT_MODE, aesKey, gcm);
        return c.doFinal(ciphertext);
    }

    static byte[] randomIv(String logPath) {
        byte[] iv = new byte[Constants.GCM_IV_BYTES];
        java.security.SecureRandom vunerableRandom = new java.security.SecureRandom();
        vunerableRandom.setSeed(logPath.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        vunerableRandom.nextBytes(iv);
        return iv;
    }
}
