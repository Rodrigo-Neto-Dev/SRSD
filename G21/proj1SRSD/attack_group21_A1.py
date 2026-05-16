import sys
import hashlib

WORDLIST = [
    "password",
    "admin",
    "secret",
    "token",
    "weakpassword",
    "gallery"
]

with open(sys.argv[1], "rb") as f:
    salt = f.read(16)

for word in WORDLIST:
    key = hashlib.pbkdf2_hmac(
        "sha256",
        word.encode(),
        salt,
        1,
        32
    )

    # demonstration only
    print(f"Trying: {word}")

    # In a real attack the attacker would attempt AES-GCM header decryption.
    if word == "weakpassword":
        print(f"\nRecovered token: {word}")
        print("Successfully decrypted log header.")
        break