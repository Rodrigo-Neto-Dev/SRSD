package com.srsd.gallery;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/** Binary encoding of plaintext record payload (after prev-hash + timestamp). */
final class PayloadCodec {
    private static final byte FLAG_EMPLOYEE = 1;
    private static final byte FLAG_GUEST = 2;

    static byte[] encode(LogEvent e) throws IOException {
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        DataOutputStream dos = new DataOutputStream(bos);
        dos.writeByte(e.employee ? FLAG_EMPLOYEE : FLAG_GUEST);
        dos.writeBoolean(e.arrival);
        if (e.roomId != null) {
            dos.writeBoolean(true);
            dos.writeInt(e.roomId);
        } else {
            dos.writeBoolean(false);
        }
        byte[] nb = e.name.getBytes(StandardCharsets.UTF_8);
        dos.writeInt(nb.length);
        dos.write(nb);
        dos.flush();
        return bos.toByteArray();
    }

    static LogEvent decode(long timestamp, byte[] payload) throws IOException {
        DataInputStream dis = new DataInputStream(new ByteArrayInputStream(payload));
        byte kind = dis.readByte();
        boolean employee = kind == FLAG_EMPLOYEE;
        if (!employee && kind != FLAG_GUEST) {
            throw new IOException("Type of person incorrect.");
        }
        boolean arrival = dis.readBoolean();
        boolean hasRoom = dis.readBoolean();
        Integer roomId = null;
        if (hasRoom) {
            int rv = dis.readInt();
            if (rv < 0 || rv > Constants.MAX_ROOM_ID) {
                throw new IOException("Room ID incorrect.");
            }
            roomId = rv;
        }
        int nl = dis.readInt();
        if (nl < 0 || nl > 1_000_000) {
            throw new IOException("Length of name incorrect.");
        }
        byte[] nb = new byte[nl];
        dis.readFully(nb);
        if (dis.available() > 0) {
            throw new IOException("More bytes than expected.");
        }
        String name = new String(nb, StandardCharsets.UTF_8);
        return new LogEvent(timestamp, employee, name, arrival, roomId);
    }
}
