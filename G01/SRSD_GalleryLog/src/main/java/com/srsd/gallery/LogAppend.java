package com.srsd.gallery;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.GeneralSecurityException;

public final class LogAppend {

    private LogAppend() {}

    public static void main(String[] args) {
        if (args.length == 0) {
            invalid();
            return;
        }
        boolean batch = args.length == 2 && "-B".equals(args[0]);
        if (batch) {
            String batchFile;
            if (args[1] != null) {
                batchFile = args[1];
            } else {
                invalid();
                return;
            }
            Path bf = InputValidators.safePath(batchFile);
            if (!Files.isRegularFile(bf)) {
                invalid();
                return;
            }
            try {
                String text = Files.readString(bf, StandardCharsets.UTF_8);
                String[] lines = text.split("\\R");
                for (String line : lines) {
                    line = line.trim();
                    if (line.isEmpty()) {
                        continue;
                    }
                    String[] parts = line.split("\\s+");
                    try {
                        AppendArgs a = AppendArgs.parse(parts);
                        String err = a.validate();
                        if (err != null) {
                            System.out.println("Invalid.");
                            continue;
                        }
                        if (!tryAppend(a)) {
                            System.out.println("Invalid.");
                        }
                    } catch (IllegalArgumentException e) {
                        System.out.println("Invalid.");
                    }
                }
            } catch (IOException e) {
                invalid();
                return;
            }
            System.exit(0);
            return;
        }
        try {
            AppendArgs a = AppendArgs.parse(args);
            String err = a.validate();
            if (err != null) {
                invalid();
                return;
            }
            if (!tryAppend(a)) {
                invalid();
                return;
            }
        } catch (IllegalArgumentException e) {
            invalid();
            return;
        }
        System.exit(0);
    }

    private static boolean tryAppend(AppendArgs a) {
        Path p = InputValidators.safePath(a.logPath);
        LogEvent ev = a.toEvent();
        try {
            if (!Files.exists(p)) {
                LogFile.createAndAppend(p, a.token, ev);
            } else if (Files.isRegularFile(p)) {
                LogFile.append(p, a.token, ev);
            } else {
                return false;
            }
        } catch (IllegalArgumentException e) {
            return false;
        } catch (GeneralSecurityException e) {
            return false;
        } catch (IOException e) {
            return false;
        }
        return true;
    }

    private static void invalid() {
        System.out.println("Invalid.");
        System.exit(111);
    }
}
