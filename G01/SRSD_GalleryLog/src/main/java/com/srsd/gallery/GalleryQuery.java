package com.srsd.gallery;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Collectors;

final class GalleryQuery {

    private GalleryQuery() {}

    static String queryState(List<LogEvent> events) {
        GalleryStateMachine sm = new GalleryStateMachine();
        for (LogEvent e : events) {
            if (sm.validateAndApply(e) != null) {
                throw new IllegalStateException("Old event is not correct.");
            }
        }
        Map<PersonKey, PersonLocation.State> persons = sm.getPersons();
        TreeSet<String> emp = new TreeSet<>();
        TreeSet<String> gst = new TreeSet<>();
        Map<Integer, TreeSet<String>> roomNames = new HashMap<>();
        for (Map.Entry<PersonKey, PersonLocation.State> en : persons.entrySet()) {
            PersonKey pk = en.getKey();
            PersonLocation.State st = en.getValue();
            if (st.loc == PersonLocation.OUTSIDE) {
                continue;
            }
            if (pk.employee) {
                emp.add(pk.name);
            } else {
                gst.add(pk.name);
            }
            if (st.loc == PersonLocation.IN_ROOM) {
                roomNames.computeIfAbsent(st.roomId, k -> new TreeSet<>()).add(pk.name);
            }
        }
        StringBuilder sb = new StringBuilder();
        sb.append(String.join(",", emp));
        sb.append('\n');
        sb.append(String.join(",", gst));
        List<Integer> rooms = new ArrayList<>(roomNames.keySet());
        Collections.sort(rooms);
        for (int rid : rooms) {
            sb.append('\n');
            sb.append(rid).append(": ");
            sb.append(String.join(",", roomNames.get(rid)));
        }
        return sb.toString();
    }

    static String queryRoomHistory(List<LogEvent> events, boolean employee, String name) {
        List<Integer> order = new ArrayList<>();
        for (LogEvent e : events) {
            if (e.employee == employee && e.name.equals(name) && e.roomId != null && e.arrival) {
                order.add(e.roomId);
            }
        }
        return order.stream().map(String::valueOf).collect(Collectors.joining(","));
    }

    static String queryIntersection(List<LogEvent> events, List<PersonKey> wanted) {
        if (wanted.isEmpty()) {
            return "";
        }
        Set<String> present = new HashSet<>();
        for (LogEvent e : events) {
            present.add(keyStr(e.employee, e.name));
        }
        List<PersonKey> active = new ArrayList<>();
        for (PersonKey w : wanted) {
            if (present.contains(keyStr(w.employee, w.name))) {
                active.add(w);
            }
        }
        if (active.isEmpty()) {
            return "";
        }
        Set<Integer> allRooms = new HashSet<>();
        for (LogEvent e : events) {
            if (e.roomId != null) {
                allRooms.add(e.roomId);
            }
        }
        List<Integer> sortedRooms = allRooms.stream().sorted().collect(Collectors.toList());
        List<Integer> resultRooms = new ArrayList<>();
        for (int rid : sortedRooms) {
            List<List<long[]>> perPerson = new ArrayList<>();
            boolean skipRoom = false;
            for (PersonKey pk : active) {
                List<long[]> iv = intervalsInRoom(events, pk, rid);
                if (iv.isEmpty()) {
                    skipRoom = true;
                    break;
                }
                perPerson.add(iv);
            }
            if (skipRoom) {
                continue;
            }
            if (hasNonEmptyIntersection(perPerson)) {
                resultRooms.add(rid);
            }
        }
        return resultRooms.stream().map(String::valueOf).collect(Collectors.joining(","));
    }

    private static String keyStr(boolean emp, String name) {
        return (emp ? "E:" : "G:") + name;
    }

    private static List<long[]> intervalsInRoom(List<LogEvent> all, PersonKey pk, int roomId) {
        List<long[]> out = new ArrayList<>();
        PersonLocation.State st = PersonLocation.State.outside();
        Long openEnter = null;
        for (LogEvent e : all) {
            if (e.employee != pk.employee || !e.name.equals(pk.name)) {
                continue;
            }
            if (e.roomId == null) {
                if (e.arrival) {
                    if (st.loc != PersonLocation.OUTSIDE) {
                        throw new IllegalStateException("Person A is not outside.");
                    }
                    st = PersonLocation.State.inGallery();
                } else {
                    if (st.loc != PersonLocation.IN_GALLERY) {
                        throw new IllegalStateException("Person L is not in the gallery.");
                    }
                    st = PersonLocation.State.outside();
                }
            } else {
                int r = e.roomId;
                if (e.arrival) {
                    if (st.loc != PersonLocation.IN_GALLERY) {
                        throw new IllegalStateException("Person A is not in gallery.");
                    }
                    st = PersonLocation.State.inRoom(r);
                    if (r == roomId) {
                        openEnter = e.timestamp;
                    }
                } else {
                    if (st.loc != PersonLocation.IN_ROOM || st.roomId != r) {
                        throw new IllegalStateException("Room L mismatch.");
                    }
                    if (r == roomId && openEnter != null) {
                        out.add(new long[]{openEnter, e.timestamp});
                        openEnter = null;
                    }
                    st = PersonLocation.State.inGallery();
                }
            }
        }
        if (st.loc == PersonLocation.IN_ROOM && st.roomId == roomId && openEnter != null) {
            out.add(new long[]{openEnter, Long.MAX_VALUE});
        }
        return out;
    }

    private static boolean hasNonEmptyIntersection(List<List<long[]>> lists) {
        if (lists.isEmpty()) {
            return false;
        }
        List<long[]> cur = new ArrayList<>(lists.get(0));
        for (int i = 1; i < lists.size(); i++) {
            cur = intersectIntervalLists(cur, lists.get(i));
            if (cur.isEmpty()) {
                return false;
            }
        }
        return !cur.isEmpty();
    }

    private static List<long[]> intersectIntervalLists(List<long[]> a, List<long[]> b) {
        List<long[]> out = new ArrayList<>();
        for (long[] ia : a) {
            for (long[] ib : b) {
                long lo = Math.max(ia[0], ib[0]);
                long hi = Math.min(ia[1], ib[1]);
                if (lo <= hi) {
                    out.add(new long[]{lo, hi});
                }
            }
        }
        return mergeIntervals(out);
    }

    private static List<long[]> mergeIntervals(List<long[]> raw) {
        if (raw.isEmpty()) {
            return raw;
        }
        raw.sort(java.util.Comparator.comparingLong(x -> x[0]));
        List<long[]> merged = new ArrayList<>();
        long[] cur = raw.get(0).clone();
        for (int i = 1; i < raw.size(); i++) {
            long[] x = raw.get(i);
            if (x[0] <= cur[1]) {
                cur[1] = Math.max(cur[1], x[1]);
            } else {
                merged.add(cur);
                cur = x.clone();
            }
        }
        merged.add(cur);
        return merged;
    }
}
