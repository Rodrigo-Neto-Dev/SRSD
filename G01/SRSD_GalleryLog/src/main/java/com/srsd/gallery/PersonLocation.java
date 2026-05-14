package com.srsd.gallery;

import java.util.Objects;

enum PersonLocation {
    OUTSIDE,
    IN_GALLERY,
    IN_ROOM;

    static final class State {
        PersonLocation loc;
        /** Only meaningful when loc == IN_ROOM. */
        int roomId;

        State(PersonLocation loc, int roomId) {
            this.loc = loc;
            this.roomId = roomId;
        }

        static State outside() {
            return new State(OUTSIDE, -1);
        }

        static State inGallery() {
            return new State(IN_GALLERY, -1);
        }

        static State inRoom(int rid) {
            return new State(IN_ROOM, rid);
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (!(o instanceof State state)) return false;
            return roomId == state.roomId && loc == state.loc;
        }

        @Override
        public int hashCode() {
            return Objects.hash(loc, roomId);
        }
    }
}
