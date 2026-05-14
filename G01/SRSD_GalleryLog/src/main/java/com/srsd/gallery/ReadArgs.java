package com.srsd.gallery;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

final class ReadArgs {
    String token;
    boolean hasS;
    boolean hasR;
    boolean hasI;
    final List<PersonKey> persons = new ArrayList<>();
    String logPath;

    static ReadArgs parse(String[] args) throws IllegalArgumentException {
        if (args.length == 0) {
            throw new IllegalArgumentException();
        }

        ReadArgs ra = new ReadArgs();
        int sCount = 0, rCount = 0, iCount = 0;
        List<String> loose = new ArrayList<>();

        Set<PersonKey> seenPersons = new HashSet<>();
        List<PersonKey> tempPersons = new ArrayList<>();

        for (int idx = 0; idx < args.length; ) {
            String a = args[idx];
            switch (a) {
                case "-K" -> {
                    if (idx + 1 >= args.length || ra.token != null) throw new IllegalArgumentException("Missing or dup K.");
                    ra.token = args[idx + 1];
                    idx += 2;
                }
                case "-S" -> {
                    ra.hasS = true;
                    sCount++;
                    idx++;
                }
                case "-R" -> {
                    ra.hasR = true;
                    rCount++;
                    idx++;
                }
                case "-I" -> {
                    ra.hasI = true;
                    iCount++;
                    idx++;
                }
                case "-E", "-G" -> {
                    if (idx + 1 >= args.length) throw new IllegalArgumentException("Missing name.");
                    String name = args[idx + 1];

                    if (!InputValidators.isValidName(name)) {
                        throw new IllegalArgumentException("Invalid name format.");
                    }

                    PersonKey pk = new PersonKey(a.equals("-E"), name);

                    if (!seenPersons.add(pk)) {
                        throw new IllegalArgumentException("Duplicate person: " + name);
                    }

                    tempPersons.add(pk);
                    idx += 2;
                }
                default -> {
                    loose.add(a);
                    idx++;
                }
            }
        }


        if (!InputValidators.isValidToken(ra.token)) {
            throw new IllegalArgumentException("Invalid or missing token.");
        }

        if (sCount + rCount + iCount != 1) {
            throw new IllegalArgumentException("Exactly one mode (-S, -R, -I) must be specified.");
        }

        if (loose.size() != 1) {
            throw new IllegalArgumentException("Missing or too many loose arguments (log path).");
        }

        ra.logPath = loose.get(0);
        if (!InputValidators.isValidLogPath(ra.logPath)) {
            throw new IllegalArgumentException("Invalid log path.");
        }

        if (ra.hasS) {
            if (!tempPersons.isEmpty()) throw new IllegalArgumentException("State mode (-S) cannot take person arguments.");
        } else if (ra.hasR) {
            if (tempPersons.size() != 1) throw new IllegalArgumentException("History mode (-R) requires exactly ONE person.");
            ra.persons.addAll(tempPersons);
        } else if (ra.hasI) {
            if (tempPersons.isEmpty()) throw new IllegalArgumentException("Intersection mode (-I) requires AT LEAST ONE person.");
            ra.persons.addAll(tempPersons);
        }

        return ra;
    }
}