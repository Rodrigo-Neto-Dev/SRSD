from hashlib import pbkdf2_hmac
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
import struct, sys

def crack_and_dump(logfile, wordlist):
    with open(logfile, "rb") as f:
        data = f.read()

    salt = data[:16]
    header_iv = data[16:28]
    header_ct = data[28:60]
    header_tag = data[60:76]

    print(f"[*] Salt: {salt.hex()}")
    print(f"[*] Trying wordlist...")

    # Step 1: crack the token
    found_token = None
    found_key = None

    with open(wordlist, "r", encoding="latin-1") as wl:
        for line in wl:
            token = line.strip()
            key = pbkdf2_hmac('sha256', token.encode(), salt, 1, dklen=32)
            try:
                AESGCM(key).decrypt(header_iv, header_ct + header_tag, salt)
                found_token = token
                found_key = key
                break
            except:
                continue

    if not found_token:
        print("[-] Token not found in wordlist.")
        return

    print(f"[+] Token cracked: '{found_token}'")
    print(f"[+] Key: {found_key.hex()}")
    print()

    # Step 2: read total_entries from decrypted header
    header_plain = AESGCM(found_key).decrypt(header_iv, header_ct + header_tag, salt)
    total_entries = struct.unpack_from('<Q', header_plain, 0)[0]
    last_timestamp = struct.unpack_from('<Q', header_plain, 8)[0]
    print(f"[*] Total entries in log: {total_entries}")
    print(f"[*] Last timestamp: {last_timestamp}")
    print()

    # Step 3: iterate over every entry
    offset = 76  # 16 (salt) + 60 (HeaderBlock)
    aesgcm = AESGCM(found_key)

    for i in range(total_entries):
        # EntryMetadata: seq(4) + prev_tag(16) + plaintext_len(4) + iv(12) = 36 bytes
        meta = data[offset:offset+36]
        seq = struct.unpack_from('<I', meta, 0)[0]
        prev_tag = meta[4:20]
        plaintext_len = struct.unpack_from('<I', meta, 20)[0]
        entry_iv = meta[24:36]
        offset += 36

        ciphertext = data[offset:offset+plaintext_len]
        offset += plaintext_len

        entry_tag = data[offset:offset+16]
        offset += 16

        # AAD = seq + prev_tag
        aad = struct.pack('<I', seq) + prev_tag

        try:
            plaintext = aesgcm.decrypt(entry_iv, ciphertext + entry_tag, aad)
            print(f"  Entry {seq}: {plaintext.decode().strip(chr(0))}")
        except Exception as e:
            print(f"  Entry {seq}: DECRYPTION FAILED — {e}")

if __name__ == "__main__":
    crack_and_dump("log1", "/usr/share/wordlists/rockyou.txt")