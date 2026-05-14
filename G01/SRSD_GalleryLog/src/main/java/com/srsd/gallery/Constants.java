package com.srsd.gallery;

final class Constants {
    static final byte[] BEGIN_LOG = {'B', 'L', 'O', 'G'};
    static final byte FORMAT_VERSION = 1;
    static final int SALT_BYTES = 16;

    static final int PBKDF2_ITERATIONS = 120_000;

    static final int AES_KEY_BITS = 256;
    static final int GCM_IV_BYTES = 12;
    static final int GCM_TAG_BITS = 128;
    static final int PREV_HASH_BYTES = 32;
    static final long MAX_TIMESTAMP = 1_073_741_823L;
    static final long MAX_ROOM_ID = 1_073_741_823L;

    private Constants() {}
}
