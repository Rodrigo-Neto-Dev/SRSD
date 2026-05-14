package com.srsd.gallery;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.GeneralSecurityException;
import java.util.List;

public final class LogRead {

    private LogRead() {}

    public static void main(String[] args) {
        ReadArgs ra;
        try {
            ra = ReadArgs.parse(args);
        } catch (IllegalArgumentException e) {
            System.out.println("Invalid");
            System.exit(111);
            return;
        }
        Path p = InputValidators.safePath(ra.logPath);
        if (!Files.isRegularFile(p)) {
            System.out.println("Integrity violation");
            System.exit(111);
            return;
        }
        List<LogEvent> events;
        try {
            LogData fullData = LogFile.loadFullData(p,ra.token);
            events = fullData.events;
        } catch (IOException | GeneralSecurityException e) {
            System.out.println("Integrity violation");
            System.exit(111);
            return;
        }
        try {
            if (ra.hasS) {
                System.out.println(GalleryQuery.queryState(events));
            } else if (ra.hasR) {
                PersonKey pk = ra.persons.get(0);
                String out = GalleryQuery.queryRoomHistory(events, pk.employee, pk.name);
                if (!out.isEmpty()) {
                    System.out.println(out);
                }
            } else {
                String out = GalleryQuery.queryIntersection(events, ra.persons);
                if (!out.isEmpty()) {
                    System.out.println(out);
                }
            }
        } catch (IllegalStateException e) {
            System.out.println("Integrity violation");
            System.exit(111);
            return;
        }
        System.exit(0);
    }
}
