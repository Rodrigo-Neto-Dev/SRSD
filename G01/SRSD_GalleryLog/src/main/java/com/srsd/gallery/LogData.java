package com.srsd.gallery;

import java.util.List;


final class LogData {
    final List<LogEvent> events;
    final byte[] salt;
    final byte[] lastRecordBody;

    LogData(List<LogEvent> events, byte[] salt, byte[] lastRecordBody) {
        this.events = events;
        this.salt = salt;
        this.lastRecordBody = lastRecordBody;
    }
}