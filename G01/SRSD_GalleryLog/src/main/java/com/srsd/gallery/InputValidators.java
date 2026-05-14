package com.srsd.gallery;

import java.nio.file.Path;
import java.util.regex.Pattern;

final class InputValidators {
    private static final Pattern TOKEN = Pattern.compile("^[a-zA-Z0-9]+$");
    private static final Pattern NAME = Pattern.compile("^[a-zA-Z]+$");
    private static final Pattern ROOM_DIGITS = Pattern.compile("^[0-9]+$");
    /** Log filename/path per spec: alphanumeric, underscores, periods, slashes. */
    private static final Pattern LOG_PATH = Pattern.compile("^[a-zA-Z0-9_.\\/\\\\:\\-]+$");

    private InputValidators() {}

    static boolean isValidToken(String s) {
        return s != null && !s.isEmpty() && TOKEN.matcher(s).matches();
    }

    static boolean isValidName(String s) {
        return s != null && !s.isEmpty() && NAME.matcher(s).matches();
    }

    static boolean isValidLogPath(String s) {
        return s != null && !s.isEmpty() && LOG_PATH.matcher(s).matches();
    }

    /**
     * Parses room id string; drops leading zeros by numeric parse.
     * @return null if invalid
     */
    static Integer parseRoomId(String s) {
        if (s == null || s.isEmpty() || !ROOM_DIGITS.matcher(s).matches()) {
            return null;
        }
        try {
            long v = Long.parseLong(s);
            if (v < 0 || v > Constants.MAX_ROOM_ID) {
                return null;
            }
            return (int) v;
        } catch (NumberFormatException e) {
            return null;
        }
    }

    static boolean isValidTimestamp(long t) {
        return t >= 1 && t <= Constants.MAX_TIMESTAMP;
    }

    static Path safePath(String logPath) {
        return Path.of(logPath).normalize();
    }
}
