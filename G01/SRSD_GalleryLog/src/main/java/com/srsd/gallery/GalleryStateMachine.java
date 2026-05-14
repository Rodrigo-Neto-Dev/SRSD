package com.srsd.gallery;

import java.util.HashMap;
import java.util.Map;

/**
 * Validates events against gallery rules and tracks per-person state.
 */
final class GalleryStateMachine {
    private final Map<PersonKey, PersonLocation.State> persons = new HashMap<>();
    private long lastTimestamp = -1L;

    long lastTimestamp() {
        return lastTimestamp;
    }

    /** @return null if valid, else error reason for logging */
    String validateAndApply(LogEvent e) {
        if (e.timestamp < 1 || e.timestamp > Constants.MAX_TIMESTAMP) {
            return "Bad timestamp.";
        }
        if (lastTimestamp >= 0 && e.timestamp <= lastTimestamp) {
            return "Wrong timestamp order.";
        }
        PersonKey key = new PersonKey(e.employee, e.name);
        PersonLocation.State st = persons.getOrDefault(key, PersonLocation.State.outside());

        if (e.roomId == null) {
            if (e.arrival) {
                // gallery arrival
                if (st.loc != PersonLocation.OUTSIDE) {
                    return "Person A is not outside.";
                }
                persons.put(key, PersonLocation.State.inGallery());
            } else {
                // gallery departure
                if (st.loc != PersonLocation.IN_GALLERY) {
                    return "Person L is not in the gallery.";
                }
                persons.put(key, PersonLocation.State.outside());
            }
        } else {
            int rid = e.roomId;
            if (e.arrival) {
                if (st.loc != PersonLocation.IN_GALLERY) {
                    return "Person A is not in gallery";
                }
                persons.put(key, PersonLocation.State.inRoom(rid));
            } else {
                if (st.loc != PersonLocation.IN_ROOM || st.roomId != rid) {
                    return "Room L mismatch";
                }
                persons.put(key, PersonLocation.State.inGallery());
            }
        }
        lastTimestamp = e.timestamp;
        return null;
    }

    Map<PersonKey, PersonLocation.State> getPersons() {
        return new HashMap<>(persons);
    }
}
