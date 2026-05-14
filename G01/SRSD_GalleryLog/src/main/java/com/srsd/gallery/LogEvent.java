package com.srsd.gallery;

import java.util.Objects;

/** Immutable decoded log event (plaintext semantics). */
final class LogEvent {
    final long timestamp;
    final boolean employee;
    final String name;
    /** True = arrival, false = departure. */
    final boolean arrival;
    /** Null = gallery-wide; non-null = room event. */
    final Integer roomId;

    LogEvent(long timestamp, boolean employee, String name, boolean arrival, Integer roomId) {
        this.timestamp = timestamp;
        this.employee = employee;
        this.name = Objects.requireNonNull(name);
        this.arrival = arrival;
        this.roomId = roomId;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof LogEvent that)) return false;
        return timestamp == that.timestamp
                && employee == that.employee
                && arrival == that.arrival
                && name.equals(that.name)
                && Objects.equals(roomId, that.roomId);
    }

    @Override
    public int hashCode() {
        return Objects.hash(timestamp, employee, name, arrival, roomId);
    }
}
