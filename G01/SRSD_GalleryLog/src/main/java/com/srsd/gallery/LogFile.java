package com.srsd.gallery;

import java.io.*;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.*;
import java.security.GeneralSecurityException;
import java.security.SecureRandom;
import java.util.*;

final class LogFile {
    private static final int MAX_RECORD_SIZE = 16 * 1024 * 1024;
    private static final SecureRandom RANDOM = new SecureRandom();

    private LogFile() {}

    static boolean exists(Path path) {
        return Files.isRegularFile(path);
    }

    private static byte[] generateHeader() {
        byte[] salt = new byte[Constants.SALT_BYTES];
        RANDOM.nextBytes(salt);
        ByteBuffer header = ByteBuffer.allocate(Constants.BEGIN_LOG.length + 1 + salt.length);
        header.put(Constants.BEGIN_LOG).put(Constants.FORMAT_VERSION).put(salt);
        return header.array();
    }

    private static byte[] extractSalt(byte[] header) throws IOException {
        int expectedSize = Constants.BEGIN_LOG.length + 1 + Constants.SALT_BYTES;
        if (header.length != expectedSize) {
            throw new IOException("Invalid log header: size is not correct.");
        }
        byte[] begin_log = Arrays.copyOfRange(header, 0, Constants.BEGIN_LOG.length);
        if (!Arrays.equals(begin_log, Constants.BEGIN_LOG)) {
            throw new IOException("Invalid log file: initial log value missing.");
        }
        if (header[Constants.BEGIN_LOG.length] != Constants.FORMAT_VERSION) {
            throw new IOException("Unsupported log version.");
        }
        return Arrays.copyOfRange(header, Constants.BEGIN_LOG.length + 1, header.length);
    }

    static LogData loadFullData(Path path, String token) throws IOException, GeneralSecurityException {
        byte[] data = Files.readAllBytes(path);
        int headerSize = Constants.BEGIN_LOG.length + 1 + Constants.SALT_BYTES;

        if (data.length < headerSize) throw new IOException("Log file is truncated or corrupted.");

        byte[] salt = extractSalt(Arrays.copyOfRange(data, 0, headerSize));
        CryptoService crypto = new CryptoService(token, salt);

        List<LogEvent> events = new ArrayList<>();
        byte[] lastRecordBody = null;

        try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(data, headerSize, data.length - headerSize))) {
            while (in.available() > 0) {
                int len = in.readInt();
                if (len <= 0 || len > MAX_RECORD_SIZE) throw new IOException("Illegal record length detected.");

                byte[] body = new byte[len];
                in.readFully(body);

                RecordCodec.DecodedRecord decoded = RecordCodec.decodeRecord(crypto, body);

                byte[] expectedPrevHash = (lastRecordBody == null)
                        ? new byte[Constants.PREV_HASH_BYTES]
                        : RecordCodec.hashRecordBytes(lastRecordBody);

                if (!Arrays.equals(decoded.prevHashDeclared, expectedPrevHash)) {
                    throw new IOException("Log integrity compromised: hash chain broken.");
                }

                events.add(decoded.event);
                lastRecordBody = body;
            }
        }
        return new LogData(events, salt, lastRecordBody);
    }

    static void append(Path path, String token, LogEvent newEvent) throws IOException, GeneralSecurityException {
        LogData logData = loadFullData(path, token);

        GalleryStateMachine sm = new GalleryStateMachine();
        for (LogEvent e : logData.events) {
            String error = sm.validateAndApply(e);
            if (error != null) {
                throw new IllegalArgumentException("Integrity violation: Old event is not valid. " + error);
            }
        }
        if (sm.validateAndApply(newEvent) != null) {
            throw new IllegalArgumentException("New event is invalid.");
        }

        CryptoService crypto = new CryptoService(token, logData.salt);
        byte[] prevHash = (logData.lastRecordBody == null)
                ? new byte[Constants.PREV_HASH_BYTES]
                : RecordCodec.hashRecordBytes(logData.lastRecordBody);

        byte[] recordData = RecordCodec.encodeRecord(crypto, prevHash, newEvent,path.toString());

        try (FileChannel channel = FileChannel.open(path, StandardOpenOption.APPEND)) {
            ByteBuffer buffer = ByteBuffer.allocate(4 + recordData.length);
            buffer.putInt(recordData.length);
            buffer.put(recordData);
            buffer.flip();
            while(buffer.hasRemaining()) channel.write(buffer);
        }
    }

    static void createAndAppend(Path path, String token, LogEvent firstEvent) throws IOException, GeneralSecurityException {
        GalleryStateMachine sm = new GalleryStateMachine();
        if (sm.validateAndApply(firstEvent) != null) {
            throw new IllegalArgumentException("Event not valid");
        }

        byte[] header = generateHeader();
        byte[] salt = extractSalt(header);
        CryptoService crypto = new CryptoService(token, salt);

        byte[] firstRecord = RecordCodec.encodeRecord(crypto, new byte[Constants.PREV_HASH_BYTES], firstEvent,path.toString());

        if (path.getParent() != null) Files.createDirectories(path.getParent());

        try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(Files.newOutputStream(path, StandardOpenOption.CREATE_NEW)))) {
            out.write(header);
            out.writeInt(firstRecord.length);
            out.write(firstRecord);
        }
    }
}