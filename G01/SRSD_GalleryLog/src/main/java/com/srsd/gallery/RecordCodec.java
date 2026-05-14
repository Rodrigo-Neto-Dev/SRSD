package com.srsd.gallery;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.security.GeneralSecurityException;
import java.security.MessageDigest;
import java.util.Arrays;

/**
 * One stored record: length || IV(12) || AES-GCM ciphertext.
 * Plaintext: prevHash(32) || timestamp(8) || payload.
 */
final class RecordCodec {
    private RecordCodec() {}

    static byte[] encodeRecord(CryptoService crypto, byte[] prevHash, LogEvent event,String logPath)
            throws IOException, GeneralSecurityException {
        byte[] iv = CryptoService.randomIv(logPath);
        byte[] payload = PayloadCodec.encode(event);
        ByteArrayOutputStream pt = new ByteArrayOutputStream();
        DataOutputStream dos = new DataOutputStream(pt);
        dos.write(prevHash);
        dos.writeLong(event.timestamp);
        dos.write(payload);
        dos.flush();
        byte[] plain = pt.toByteArray();
        byte[] ct = crypto.encrypt(plain, iv);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        DataOutputStream w = new DataOutputStream(out);
        w.write(iv);
        w.write(ct);
        w.flush();
        return out.toByteArray();
    }

    static DecodedRecord decodeRecord(CryptoService crypto, byte[] recordBytes)
            throws IOException, GeneralSecurityException {
        if (recordBytes.length < Constants.GCM_IV_BYTES + 16) {
            throw new IOException("Record is too short.");
        }
        byte[] iv = Arrays.copyOfRange(recordBytes, 0, Constants.GCM_IV_BYTES);
        byte[] ct = Arrays.copyOfRange(recordBytes, Constants.GCM_IV_BYTES, recordBytes.length);
        byte[] plain = crypto.decrypt(ct, iv);
        DataInputStream dis = new DataInputStream(new ByteArrayInputStream(plain));
        byte[] prev = new byte[Constants.PREV_HASH_BYTES];
        dis.readFully(prev);
        long ts = dis.readLong();
        byte[] rest = dis.readAllBytes();
        LogEvent ev = PayloadCodec.decode(ts, rest);
        return new DecodedRecord(prev, ev, recordBytes);
    }

    static byte[] hashRecordBytes(byte[] recordBytes) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(recordBytes);
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    static final class DecodedRecord {
        final byte[] prevHashDeclared;
        final LogEvent event;
        /** Raw bytes of this record as stored (for chaining). */
        final byte[] rawRecordBytes;

        DecodedRecord(byte[] prevHashDeclared, LogEvent event, byte[] rawRecordBytes) {
            this.prevHashDeclared = prevHashDeclared;
            this.event = event;
            this.rawRecordBytes = rawRecordBytes;
        }
    }
}
