package com.srsd.gallery;

import java.util.ArrayList;
import java.util.List;

final class AppendArgs {
    Long timestamp;
    String token;
    Boolean isEmployee;
    String personName;
    Integer roomId;
    Boolean arrival;
    String logPath;

    /** @return null if OK, else error */
    String validate() {
        if (timestamp == null || token == null || arrival == null || logPath == null) {
            return "incomplete";
        }
        if (!InputValidators.isValidToken(token)) {
            return "token";
        }
        if (!InputValidators.isValidLogPath(logPath)) {
            return "logpath";
        }
        if (!InputValidators.isValidTimestamp(timestamp)) {
            return "ts";
        }
        if (isEmployee == null) {
            return "eg";
        }
        if (!InputValidators.isValidName(personName)) {
            return "name";
        }
        return null;
    }

    LogEvent toEvent() {
        return new LogEvent(timestamp, isEmployee, personName, arrival, roomId);
    }

    static AppendArgs parse(String[] args) throws IllegalArgumentException {
        AppendArgs o = new AppendArgs();
        List<String> loose = new ArrayList<>();

        for (int i = 0; i < args.length; ) {
            String a = args[i];
            switch (a) {
                case "-T" -> {
                    if (i + 1 >= args.length) throw new IllegalArgumentException();
                    o.timestamp = parseLong(args[i + 1]);
                    if (o.timestamp == null) throw new IllegalArgumentException();
                    i += 2;
                }
                case "-K" -> {
                    if (i + 1 >= args.length) throw new IllegalArgumentException();
                    o.token = args[i + 1];
                    i += 2;
                }
                case "-E", "-G" -> {
                    if (i + 1 >= args.length) throw new IllegalArgumentException();
                    if (o.isEmployee != null) throw new IllegalArgumentException("Duplicate person flag.");
                    o.isEmployee = a.equals("-E");
                    o.personName = args[i + 1];
                    i += 2;
                }
                case "-R" -> {
                    if (i + 1 >= args.length) throw new IllegalArgumentException();
                    o.roomId = InputValidators.parseRoomId(args[i + 1]);
                    if (o.roomId == null) throw new IllegalArgumentException();
                    i += 2;
                }
                case "-A" -> {
                    o.arrival = true;
                    i++;
                }
                case "-L" -> {
                    o.arrival = false;
                    i++;
                }
                case "-B" -> throw new IllegalArgumentException("Batch in line.");
                default -> {
                    loose.add(a);
                    i++;
                }
            }
        }

        if (loose.size() != 1) {
            throw new IllegalArgumentException();
        }
        o.logPath = loose.get(0);

        return o;
    }

    private static Long parseLong(String s) {
        try {
            return Long.parseLong(s);
        } catch (NumberFormatException e) {
            return null;
        }
    }
}