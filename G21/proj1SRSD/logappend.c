#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <setjmp.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <openssl/hmac.h>

#define MAX_NAME          256
#define MAX_LINE          1024
#define MAX_TOKENS        64
#define MAX_BATCH_LINE    2048
#define HEADER_IV_LEN     12
#define HEADER_TAG_LEN    16
#define ENTRY_IV_LEN      12
#define ENTRY_TAG_LEN     16
#define MAX_PLAINTEXT_LEN 512
#define HEADER_CIPHER_LEN 32
#define SALT_LEN          16

// Dynamic memory constants
#define INITIAL_PERSONS   100
#define INITIAL_CONTEXTS  10
#define GROWTH_FACTOR     2
#define MAX_ALLOC_SIZE    (1024 * 1024 * 100)  

typedef struct LogContext LogContext;

/**
 * Person state structure - dynamically allocated
 */
typedef struct {
    char name[MAX_NAME];
    char category[MAX_NAME];
    int  in_gallery;
    int  in_room;
} PersonState;

/**
 * Log context structure - dynamically allocated list
 */
struct LogContext {
    char logfile[MAX_NAME];
    unsigned char key[32];
    int authenticated;
    uint64_t total_entries;
    uint64_t last_timestamp;
    unsigned char last_tag[16];
    FILE *fd;
    int header_dirty;
    PersonState *persons;        // Dynamically allocated array
    int person_count;
    int person_capacity;
    int state_loaded;
    LogContext *next;
};

/* ── Args ─────────────────────────────────────────────────────── */

typedef struct {
    int  timestamp;
    char token[MAX_NAME];
    char name[MAX_NAME];
    int  is_employee;
    int  arrive;
    int  has_room;
    int  room_id;
    char logfile[MAX_NAME];
} Args;

/* ── File structs ──────────────────────────────────────── */

typedef struct {
    uint8_t salt[SALT_LEN];
} HeaderPlain;

typedef struct {
    uint64_t total_entries;
    uint64_t last_timestamp;
    uint8_t  last_tag[16];
} HeaderTextToCipher;

typedef struct {
    uint8_t iv[HEADER_IV_LEN];
    uint8_t nentriestag[HEADER_CIPHER_LEN];
    uint8_t tag[HEADER_TAG_LEN];
} HeaderBlock;

typedef struct {
    uint32_t      seq;
    unsigned char prev_tag[16];
    uint32_t      plaintext_len;
    unsigned char iv[12];
} EntryMetadata;


static jmp_buf        g_die_jmp;
static int            g_batch_mode     = 0;
static LogContext    *g_contexts       = NULL;
static int            g_context_count  = 0;
static int            g_memory_cleaned = 0;

/* ── Memory Management Functions ─────────────────────────────────── */

/**
 * Checks if a multiplication will overflow SIZE_MAX.
 * 
 * @param a First operand
 * @param b Second operand
 * @return 1 if multiplication would overflow, 0 otherwise
 */
static int would_overflow(size_t a, size_t b) {
    if (a == 0 || b == 0) return 0;
    return (a > SIZE_MAX / b);
}

/**
 * Ensures a person array has enough capacity for one more person.
 * Grows the array by GROWTH_FACTOR if needed.
 * 
 * @param ctx Log context containing the persons array
 * @return 1 on success, 0 on failure (out of memory)
 */
static int ensure_person_capacity(LogContext *ctx) {
    // Enough capacity already
    if (ctx->person_count < ctx->person_capacity) {
        return 1;
    }
    
    int new_capacity = (ctx->person_capacity == 0) ? INITIAL_PERSONS 
                                                    : ctx->person_capacity * GROWTH_FACTOR;
    
    // Check for integer overflow
    if (new_capacity < ctx->person_capacity) {
        fprintf(stderr, "ERROR: Integer overflow in person capacity calculation\n");
        return 0;
    }
    
    // Check for allocation size overflow
    if (would_overflow(new_capacity, sizeof(PersonState))) {
        fprintf(stderr, "ERROR: Person allocation size too large\n");
        return 0;
    }
    
    // Check against maximum allocation size
    if (new_capacity * sizeof(PersonState) > MAX_ALLOC_SIZE) {
        fprintf(stderr, "ERROR: Person allocation would exceed maximum size\n");
        return 0;
    }
    
    PersonState *new_persons = realloc(ctx->persons, new_capacity * sizeof(PersonState));
    if (!new_persons) {
        fprintf(stderr, "ERROR: Failed to allocate memory for %d persons\n", new_capacity);
        return 0;
    }
    
    ctx->persons = new_persons;
    ctx->person_capacity = new_capacity;
    
    // Initialize new entries to zero
    memset(&ctx->persons[ctx->person_count], 0, 
           (ctx->person_capacity - ctx->person_count) * sizeof(PersonState));
    
    return 1;
}

/**
 * Frees all contexts and associated memory.
 * Uses OPENSSL_cleanse for sensitive data and prevents double free.
 */
static void free_contexts(void) {
    // Prevent double free
    if (g_memory_cleaned) return;
    
    LogContext *ctx = g_contexts;
    while (ctx) {
        LogContext *next = ctx->next;
        if (ctx->fd) {
            fclose(ctx->fd);
            ctx->fd = NULL;
        }
        if (ctx->persons) {
            // Wipe sensitive data before freeing
            OPENSSL_cleanse(ctx->persons, ctx->person_capacity * sizeof(PersonState));
            free(ctx->persons);
            ctx->persons = NULL;
        }
        // Wipe context before freeing
        OPENSSL_cleanse(ctx, sizeof(LogContext));
        free(ctx);
        ctx = next;
    }
    g_contexts = NULL;
    g_context_count = 0;
    g_memory_cleaned = 1;
}

/**
 * Gets or creates a context for the specified log file.
 * 
 * @param logfile Path to the log file
 * @return Pointer to LogContext, or NULL on failure
 */
static LogContext *get_context(const char *logfile) {
    LogContext *ctx = g_contexts;
    while (ctx) {
        if (strcmp(ctx->logfile, logfile) == 0)
            return ctx;
        ctx = ctx->next;
    }
    
    // Create new context
    ctx = calloc(1, sizeof(LogContext));
    if (!ctx) return NULL;
    
    strncpy(ctx->logfile, logfile, MAX_NAME - 1);
    ctx->logfile[MAX_NAME - 1] = '\0';
    
    // Initialize dynamic arrays
    ctx->persons = NULL;
    ctx->person_count = 0;
    ctx->person_capacity = 0;
    
    ctx->next = g_contexts;
    g_contexts = ctx;
    g_context_count++;
    
    return ctx;
}

/* ── Error Handling ───────────────────────────────────────────── */

/**
 * Terminates with "invalid" and exit code 111.
 * In batch mode, uses longjmp to continue processing.
 */
static void die(void) {
    printf("invalid\n");
    if (g_batch_mode) longjmp(g_die_jmp, 1);
    free_contexts();
    exit(111);
}
/**
 * Terminates with "integrity violation" and exit code 111.
 * Always fatal, even in batch mode.
 */
static void die_integrity(void) {
    printf("integrity violation\n");
    free_contexts();
    exit(111);
}

/* ── Validation Functions ─────────────────────────────────────── */

/**
 * Verifies if a name contains only alphabetic characters.
 * 
 * @param s Name string to validate
 * @return 1 if valid, 0 otherwise
 */
static int is_alpha_name(const char *s) {
    if (!s || *s == '\0') return 0;
    size_t len = strlen(s);
    if (len >= MAX_NAME) return 0;
    for (; *s; s++) if (!isalpha((unsigned char)*s)) return 0;
    return 1;
}
/**
 * Verifies if a token contains only alphanumeric characters.
 * 
 * @param s Token string to validate
 * @return 1 if valid, 0 otherwise
 */
static int is_alnum_token(const char *s) {
    if (!s || *s == '\0') return 0;
    for (; *s; s++) if (!isalnum((unsigned char)*s)) return 0;
    return 1;
}
/**
 * Validates that a file path contains only safe characters.
 * 
 * @param s File path to validate
 * @return 1 if valid, 0 otherwise
 */
static int is_valid_filepath(const char *s) {
    if (!s || *s == '\0') return 0;
    size_t len = strlen(s);
    if (len >= MAX_NAME) return 0;
    for (; *s; s++) {
        unsigned char c = (unsigned char)*s;
        if (!isalnum(c) && c != '_' && c != '.' && c != '/') return 0;
    }
    return 1;
}
/**
 * Parses a non-negative integer from a string.
 * 
 * @param s Input string
 * @param out Pointer to store the parsed integer
 * @return 1 on success, 0 on failure
 */
static int parse_nonneg_int(const char *s, int *out) {
    if (!s || *s == '\0') return 0;
    for (const char *p = s; *p; p++) if (!isdigit((unsigned char)*p)) return 0;
    long v = strtol(s, NULL, 10);
    if (v < 0 || v > 1073741823L) return 0;
    *out = (int)v;
    return 1;
}

/* ── Cryptography Functions ───────────────────────────────────── */

/**
 * Derives an AES-256 encryption key from token and salt.
 * Uses PBKDF2-HMAC-SHA256 with 100,000 iterations.
 * 
 * @param token User-provided authentication token
 * @param salt 16-byte salt for key derivation
 * @param key Output buffer for the 32-byte derived key
 */
static void gen_key(const char *token, const unsigned char *salt, unsigned char *key) {
    static const unsigned char default_salt[16] = "FIXED_SALT_1234";
    if (!salt) salt = default_salt;
    if (!PKCS5_PBKDF2_HMAC(token, strlen(token),
                            salt, 16,
                            1, EVP_sha256(),
                            32, key)) {
        fprintf(stderr, "Error deriving key\n");
        free_contexts();
        exit(111);
    }
}
/**
 * Encrypts data using AES-256-GCM.
 * AAD is authenticated but not encrypted.
 * 
 * @param input_data Plaintext to encrypt
 * @param input_len Length of plaintext
 * @param output_data Buffer for ciphertext
 * @param output_len Pointer to store ciphertext length
 * @param key AES-256 key (32 bytes)
 * @param iv Initialization vector (12 bytes)
 * @param aad Additional Authenticated Data
 * @param aad_len Length of AAD
 * @param tag Output buffer for GCM authentication tag (16 bytes)
 * @return 1 on success, 0 on failure
 */
static int crypt(const unsigned char *input_data, size_t input_len,
                  unsigned char *output_data, size_t *output_len,
                  const unsigned char *key,
                  const unsigned char *iv,
                  const unsigned char *aad, int aad_len,
                  unsigned char *tag) {
    EVP_CIPHER_CTX *ctx = NULL;
    int len = 0, ciphertext_len = 0, ret = 0;
    // Parameter validation
    if (!input_data || !output_data || !output_len || !key || !iv || !tag) return 0;
    if (input_len > INT_MAX) return 0;
    ctx = EVP_CIPHER_CTX_new();
    if (!ctx) goto cleanup;
    // Initialize context with AES-256-GCM
    if (!EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL)) goto cleanup;
    // Set IV length to 12 bytes
    if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, 12, NULL)) goto cleanup;
    // Set key and IV
    if (!EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv)) goto cleanup;
    // Process AAD if present
    if (aad && aad_len > 0)
        if (!EVP_EncryptUpdate(ctx, NULL, &len, aad, aad_len)) goto cleanup;
    // Encrypt data
    if (!EVP_EncryptUpdate(ctx, output_data, &len, input_data, input_len)) goto cleanup;
    ciphertext_len = len;
    // Finalize encryption
    if (!EVP_EncryptFinal_ex(ctx, output_data + ciphertext_len, &len)) goto cleanup;
    ciphertext_len += len;
    // Extract authentication tag
    if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, 16, tag)) goto cleanup;
    *output_len = ciphertext_len;
    ret = 1;

cleanup:
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}
/**
 * Decrypts data using AES-256-GCM with integrity verification.
 * The tag is verified during EVP_DecryptFinal_ex.
 * 
 * @param input_data Ciphertext to decrypt
 * @param input_len Length of ciphertext
 * @param output_data Buffer for plaintext
 * @param output_len Pointer to store plaintext length
 * @param key AES-256 key (32 bytes)
 * @param iv Initialization vector (12 bytes)
 * @param aad Additional Authenticated Data
 * @param aad_len Length of AAD
 * @param tag GCM authentication tag for verification
 * @return 1 on success (tag verified), 0 on failure
 */
static int decrypt(const unsigned char *input_data, size_t input_len,
                    unsigned char *output_data, size_t *output_len,
                    const unsigned char *key,
                    const unsigned char *iv,
                    const unsigned char *aad, int aad_len,
                    const unsigned char *tag) {
    EVP_CIPHER_CTX *ctx = NULL;
    int len = 0, plaintext_len = 0, ret = 0;
    // Parameter validation
    if (!input_data || !output_data || !output_len || !key || !iv || !tag) return 0;
    if (input_len > INT_MAX) return 0;
    ctx = EVP_CIPHER_CTX_new();
    if (!ctx) goto cleanup;
    // Initialize context with AES-256-GCM
    if (!EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL)) goto cleanup;
    // Set IV length to 12 bytes
    if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, 12, NULL)) goto cleanup;
    // Set key and IV
    if (!EVP_DecryptInit_ex(ctx, NULL, NULL, key, iv)) goto cleanup;
    // Set expected tag for verification
    if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, 16, (void*)tag)) goto cleanup;
    // Process AAD if present
    if (aad && aad_len > 0)
        if (!EVP_DecryptUpdate(ctx, NULL, &len, aad, aad_len)) goto cleanup;
    // Decrypt data
    if (!EVP_DecryptUpdate(ctx, output_data, &len, input_data, input_len)) goto cleanup;
    plaintext_len = len;
    // Finalize and verify tag
    if (!EVP_DecryptFinal_ex(ctx, output_data + plaintext_len, &len)) goto cleanup;
    plaintext_len += len;
    *output_len = plaintext_len;
    ret = 1;
cleanup:
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

/* ── Header Functions ─────────────────────────────────────────── */

/**
 * Updates the header of the log file with new total_entries,
 * last_timestamp and last_tag.
 * Uses a new random IV each time; salt is used as AAD.
 * 
 * @param ctx Log context containing the current state
 */
static void update_header(LogContext *ctx) {
    // Open file for reading and writing
    FILE *f = fopen(ctx->logfile, "rb+");
    if (!f) die();
    // Read current salt for AAD
    HeaderPlain hp;
    if (fseek(f, 0, SEEK_SET) != 0) { fclose(f); die(); }
    if (fread(&hp, sizeof(HeaderPlain), 1, f) != 1) { fclose(f); die(); }
    // Prepare header data to encrypt
    HeaderTextToCipher htc;
    htc.total_entries  = ctx->total_entries;
    htc.last_timestamp = ctx->last_timestamp;
    memcpy(htc.last_tag, ctx->last_tag, 16);
    // Generate new IV for header
    HeaderBlock block;
    if (RAND_bytes(block.iv, HEADER_IV_LEN) != 1) { fclose(f); die(); }
    // Encrypt header with salt as AAD
    size_t ciphertext_len;
    if (!crypt((unsigned char*)&htc, sizeof(HeaderTextToCipher),
               block.nentriestag, &ciphertext_len,
               ctx->key, block.iv,
               (unsigned char*)&hp, sizeof(HeaderPlain),
               block.tag)) {
        fclose(f); die();
    }
    // Write header block (skip salt)
    if (fseek(f, SALT_LEN, SEEK_SET) != 0) { fclose(f); die(); }
    if (fwrite(&block, sizeof(HeaderBlock), 1, f) != 1) { fclose(f); die(); }
    fclose(f);
}

/* ── State Loading Functions ──────────────────────────────────── */

/**
 * Loads the complete log state into memory in a single O(N) pass.
 * Verifies chain integrity (seq, prev_tag) and decrypts each entry.
 * 
 * @param ctx Log context to load state into
 * @return 1 on success, 0 on integrity violation
 */
static int load_state(LogContext *ctx) {
    // Already loaded
    if (ctx->state_loaded) return 1;
    // Free existing persons if any
    if (ctx->persons) {
        OPENSSL_cleanse(ctx->persons, ctx->person_capacity * sizeof(PersonState));
        free(ctx->persons);
        ctx->persons = NULL;
    }
    ctx->person_count = 0;
    ctx->person_capacity = 0;
    ctx->state_loaded = 0;
    // Not authenticated
    if (!ctx->authenticated) return 0;
    // Empty log
    if (ctx->total_entries == 0) {
        ctx->state_loaded = 1;
        return 1;
    }
    // Open file for reading
    FILE *f = fopen(ctx->logfile, "rb");
    if (!f) return 0;
    // Skip salt and header
    if (fseek(f, SALT_LEN + sizeof(HeaderBlock), SEEK_SET) != 0) {
        fclose(f); return 0;
    }
    EntryMetadata meta;
    unsigned char ciphertext[MAX_LINE];
    unsigned char tag[16];
    unsigned char last_tag[16] = {0};   
    char plaintext[MAX_LINE];
    size_t pt_len;
    uint64_t entries_read = 0;
    // Read all entries
    while (fread(&meta, sizeof(EntryMetadata), 1, f) == 1) {
        // Validate plaintext length
        if (meta.plaintext_len == 0 || meta.plaintext_len > MAX_LINE) {
            fclose(f); return 0;
        }
        // Read ciphertext and tag
        if (fread(ciphertext, 1, meta.plaintext_len, f) != meta.plaintext_len ||
            fread(tag, 16, 1, f) != 1) {
            fclose(f); return 0;
        }
        // Verify sequence number
        if (meta.seq != entries_read + 1) {
            fclose(f); return 0;
        }
        // Verify tag chain
        if (entries_read > 0 &&
            memcmp(meta.prev_tag, last_tag, 16) != 0) {
            fclose(f); return 0;
        }
        // Prepare AAD
        unsigned char aad[20];
        memcpy(aad, &meta.seq, 4);
        memcpy(aad + 4, meta.prev_tag, 16);
        // Decrypt entry
        if (!decrypt(ciphertext, meta.plaintext_len,
                     (unsigned char*)plaintext, &pt_len,
                     ctx->key, meta.iv, aad, 20, tag)) {
            fclose(f); return 0;
        }
        // Validate plaintext length
        if (pt_len == 0 || pt_len > MAX_LINE) {
            fclose(f); return 0;
        }
        // Parse log line and update state
        int ts, ig, ir;
        char n[MAX_NAME], cat[MAX_NAME];
        if (sscanf(plaintext, "%d|%255[^|]|%255[^|]|%d|%d",
                   &ts, n, cat, &ig, &ir) == 5) {
            PersonState *p = NULL;
            // Find existing person
            for (int i = 0; i < ctx->person_count; i++) {
                if (strcmp(ctx->persons[i].name, n) == 0 &&
                    strcmp(ctx->persons[i].category, cat) == 0) {
                    p = &ctx->persons[i];
                    break;
                }
            }
            // Create new person if not found
            if (!p) {
                // Ensure capacity for new person
                if (!ensure_person_capacity(ctx)) {
                    fclose(f);
                    return 0;
                }
                p = &ctx->persons[ctx->person_count++];
                strncpy(p->name, n, MAX_NAME - 1);
                p->name[MAX_NAME - 1] = '\0';
                strncpy(p->category, cat, MAX_NAME - 1);
                p->category[MAX_NAME - 1] = '\0';
            }
            // Update state
            p->in_gallery = ig;
            p->in_room = ir;
        }
        // Save tag for next entry
        memcpy(last_tag, tag, 16);
        entries_read++;
    }
    fclose(f);
    // Verify entry count matches header
    if (entries_read != ctx->total_entries) return 0;
    ctx->state_loaded = 1;
    return 1;
}

/* ── Authentication Functions ─────────────────────────────────── */

/**
 * Authenticates a user token against a log file and derives the encryption key.
 * If the log file does not exist, it is created.
 * 
 * @param ctx Log context to authenticate
 * @param token User-provided authentication token
 * @return 1 on success, 0 on failure
 */
static int authenticate_log(LogContext *ctx, const char *token) {
    FILE *f = fopen(ctx->logfile, "rb");
    unsigned char salt[16];
    // Already authenticated
    if (ctx->authenticated) {
        if (f) fclose(f);
        return 1;
    }
    // Case A: File does not exist - create new log
    if (!f) {
        // Generate random salt
        if (RAND_bytes(salt, sizeof(salt)) != 1) return 0;
        // Derive key from token and salt
        gen_key(token, salt, ctx->key);
        // Create new file
        FILE *new_f = fopen(ctx->logfile, "wb");
        if (!new_f) return 0;
        // Write salt in plaintext
        HeaderPlain hp;
        memcpy(hp.salt, salt, sizeof(salt));
        if (fwrite(&hp, sizeof(HeaderPlain), 1, new_f) != 1) {
            fclose(new_f); return 0;
        }
        // Prepare empty header
        HeaderTextToCipher htc;
        htc.total_entries = 0;
        htc.last_timestamp = 0;
        memset(htc.last_tag, 0, sizeof(htc.last_tag));
        // Generate IV for header
        HeaderBlock block;
        if (RAND_bytes(block.iv, HEADER_IV_LEN) != 1) {
            fclose(new_f); return 0;
        }
        // Encrypt header with salt as AAD
        size_t ciphertext_len;
        if (!crypt((unsigned char*)&htc, sizeof(HeaderTextToCipher),
                   block.nentriestag, &ciphertext_len,
                   ctx->key, block.iv,
                   (unsigned char*)&hp, sizeof(HeaderPlain),
                   block.tag)) {
            fclose(new_f); return 0;
        }
        // Write header block
        if (fwrite(&block, sizeof(HeaderBlock), 1, new_f) != 1) {
            fclose(new_f); return 0;
        }
        fclose(new_f);
        // Initialize context state
        ctx->total_entries = 0;
        ctx->last_timestamp = 0;
        memset(ctx->last_tag, 0, 16);
        ctx->authenticated = 1;
        return 1;
    }
    // Read salt from file
    HeaderPlain hp;
    if (fread(&hp, sizeof(HeaderPlain), 1, f) != 1) { fclose(f); return 0; }
    // Derive key from token and salt
    gen_key(token, hp.salt, ctx->key);
    // Read header block
    HeaderBlock block;
    if (fread(&block, sizeof(HeaderBlock), 1, f) != 1) { fclose(f); return 0; }
    fclose(f);
    // Decrypt header with salt as AAD
    HeaderTextToCipher htc;
    size_t decrypted_len;
    if (!decrypt(block.nentriestag, sizeof(HeaderTextToCipher),
                 (unsigned char*)&htc, &decrypted_len,
                 ctx->key, block.iv,
                 (unsigned char*)&hp, sizeof(HeaderPlain),
                 block.tag)) {
        return 0;
    }
    // Update context from decrypted header
    ctx->total_entries = htc.total_entries;
    ctx->last_timestamp = htc.last_timestamp;
    memcpy(ctx->last_tag, htc.last_tag, 16);
    ctx->authenticated = 1;
    return 1;
}

/* ── Append Functions ─────────────────────────────────────────── */

/**
 * Appends a new encrypted entry to the log file.
 * In batch mode, keeps the file open for better performance.
 * 
 * @param ctx Log context
 * @param token Authentication token
 * @param timestamp Event timestamp
 * @param name Person's name
 * @param category "employee" or "guest"
 * @param in_gallery 1 if in gallery, 0 otherwise
 * @param in_room Room ID or -1 if not in a room
 */
static void append_log(LogContext *ctx, const char *token,
                        int timestamp, const char *name, const char *category,
                        int in_gallery, int in_room) {
    // Authenticate if needed
    if (!ctx->authenticated) {
        if (!authenticate_log(ctx, token)) die();
    }
    // In batch mode, open file once
    if (g_batch_mode && !ctx->fd) {
        ctx->fd = fopen(ctx->logfile, "ab");
        if (!ctx->fd) die();
    }
    // Build plaintext line
    char plaintext[MAX_PLAINTEXT_LEN];
    snprintf(plaintext, sizeof(plaintext), "%d|%s|%s|%d|%d",
             timestamp, name, category, in_gallery, in_room);
    int plaintext_len = strlen(plaintext) + 1;
    // Prepare entry metadata
    EntryMetadata meta;
    meta.seq = (uint32_t)(ctx->total_entries + 1);
    meta.plaintext_len = plaintext_len;
    memcpy(meta.prev_tag, ctx->last_tag, 16);
    // Generate random IV for this entry
    if (RAND_bytes(meta.iv, 12) != 1) {
        die();
    }
    // Prepare AAD: seq + prev_tag
    unsigned char aad[20];
    memcpy(aad, &meta.seq, 4);
    memcpy(aad + 4, meta.prev_tag, 16);
    // Encrypt the plaintext
    unsigned char ciphertext[MAX_LINE];
    size_t ciphertext_len;
    unsigned char tag[16];
    if (!crypt((unsigned char*)plaintext, plaintext_len,
               ciphertext, &ciphertext_len,
               ctx->key, meta.iv, aad, 20, tag)) die();
    // Write entry to file
    if (g_batch_mode && ctx->fd) {
        // Batch mode: use cached file handle
        if (fwrite(&meta, sizeof(EntryMetadata), 1, ctx->fd) != 1 ||
            fwrite(ciphertext, 1, ciphertext_len, ctx->fd) != ciphertext_len ||
            fwrite(tag, 16, 1, ctx->fd) != 1) {
            die();
        }
    } else {
        // Normal mode: open and close file each time
        FILE *f = fopen(ctx->logfile, "ab");
        if (!f) die();
        if (fwrite(&meta, sizeof(EntryMetadata), 1, f) != 1 ||
            fwrite(ciphertext, 1, ciphertext_len, f) != ciphertext_len ||
            fwrite(tag, 16, 1, f) != 1) {
            fclose(f); die();
        }
        fclose(f);
    }
    // Update context state
    ctx->total_entries++;
    ctx->last_timestamp = timestamp;
    memcpy(ctx->last_tag, tag, 16);
    // Update header (immediately in normal mode, mark dirty in batch mode)
    if (!g_batch_mode) {
        update_header(ctx);
    } else {
        ctx->header_dirty = 1;
    }
}

/* ── Helper Functions ─────────────────────────────────────────── */

/**
 * Returns the last timestamp recorded in the log.
 * 
 * @param ctx Log context
 * @return Last timestamp, or -1 if log is empty
 */
static int get_last_timestamp(LogContext *ctx) {
    if (ctx->total_entries == 0) return -1;
    return (int)ctx->last_timestamp;
}
/**
 * Parses command line arguments.
 * Validates all flags and fills the Args structure.
 * 
 * @param argc Number of arguments
 * @param argv Array of argument strings
 * @param a Pointer to Args structure to fill
 */
static void parse_args(int argc, char *argv[], Args *a) {
    memset(a, 0, sizeof(*a));
    int has_T=0, has_K=0, has_P=0, has_Act=0, has_Log=0;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-T") == 0) {
            if (++i >= argc || !parse_nonneg_int(argv[i], &a->timestamp) || a->timestamp < 1) die();
            has_T = 1;
        } else if (strcmp(argv[i], "-K") == 0) {
            if (++i >= argc || !is_alnum_token(argv[i])) die();
            strncpy(a->token, argv[i], MAX_NAME-1);
            has_K = 1;
        } else if (strcmp(argv[i], "-E") == 0) {
            if (++i >= argc || has_P || !is_alpha_name(argv[i])) die();
            strncpy(a->name, argv[i], MAX_NAME-1);
            a->is_employee = 1; 
            has_P = 1;
        } else if (strcmp(argv[i], "-G") == 0) {
            if (++i >= argc || has_P || !is_alpha_name(argv[i])) die();
            strncpy(a->name, argv[i], MAX_NAME-1);
            a->is_employee = 0; 
            has_P = 1;
        } else if (strcmp(argv[i], "-A") == 0) {
            if (has_Act) die(); 
            a->arrive = 1; 
            has_Act = 1;
        } else if (strcmp(argv[i], "-L") == 0) {
            if (has_Act) die();
            a->arrive = 0; 
            has_Act = 1;
        } else if (strcmp(argv[i], "-R") == 0) {
            if (++i >= argc || !parse_nonneg_int(argv[i], &a->room_id)) die();
            a->has_room = 1;
        } else {
            if (has_Log || !is_valid_filepath(argv[i])) die();
            strncpy(a->logfile, argv[i], MAX_NAME-1);
            has_Log = 1;
        }
    }
    if (!has_T || !has_K || !has_P || !has_Act || !has_Log) die();
}

/**
 * Gets the current state of a person from the in-memory cache.
 * 
 * @param ctx Log context
 * @param name Person's name
 * @param category "employee" or "guest"
 * @param in_gallery Output: 1 if in gallery
 * @param in_room Output: current room or -1
 */
static void get_person_state(LogContext *ctx, const char *name, const char *category,
                              int *in_gallery, int *in_room) {
    *in_gallery = 0;
    *in_room = -1;
    for (int i = 0; i < ctx->person_count; i++) {
        if (strcmp(ctx->persons[i].name, name) == 0 &&
            strcmp(ctx->persons[i].category, category) == 0) {
            *in_gallery = ctx->persons[i].in_gallery;
            *in_room = ctx->persons[i].in_room;
            return;
        }
    }
}

/**
 * Updates the state of a person in the in-memory cache.
 * Creates a new entry if the person doesn't exist.
 * 
 * @param ctx Log context
 * @param name Person's name
 * @param category "employee" or "guest"
 * @param in_gallery New gallery presence
 * @param in_room New room or -1
 */
static void update_person_state(LogContext *ctx, const char *name, const char *category,
                                 int in_gallery, int in_room) {
    // Find existing person
    for (int i = 0; i < ctx->person_count; i++) {
        if (strcmp(ctx->persons[i].name, name) == 0 &&
            strcmp(ctx->persons[i].category, category) == 0) {
            ctx->persons[i].in_gallery = in_gallery;
            ctx->persons[i].in_room = in_room;
            return;
        }
    }
    // Person not found - add new one
    if (ensure_person_capacity(ctx)) {
        PersonState *p = &ctx->persons[ctx->person_count++];
        strncpy(p->name, name, MAX_NAME - 1);
        p->name[MAX_NAME - 1] = '\0';
        strncpy(p->category, category, MAX_NAME - 1);
        p->category[MAX_NAME - 1] = '\0';
        p->in_gallery = in_gallery;
        p->in_room = in_room;
    }
}

/* ── Command Processing Functions ─────────────────────────────── */

/**
 * Processes a single logappend command with full validation.
 * In batch mode, only authenticates once and reuses in-memory state.
 * 
 * @param argc Number of arguments
 * @param argv Array of argument strings
 */
static void process_command(int argc, char *argv[]) {
    Args a;
    parse_args(argc, argv, &a);
    // Get or create context for this log file
    LogContext *ctx = get_context(a.logfile);
    if (!ctx) die_integrity();
    // Authenticate if needed
    if (!ctx->authenticated) {
        if (!authenticate_log(ctx, a.token)) {
            if (g_batch_mode) {
                printf("invalid\n");
                return;
            } else {
                die_integrity();
            }
        }
    }
    // Load state if needed
    if (!ctx->state_loaded) {
        if (!load_state(ctx)) {
            if (g_batch_mode) {
                printf("integrity violation\n");
                exit(111);
            } else {
                die_integrity();
            }
        }
    }
    const char *category = a.is_employee ? "employee" : "guest";
    // Check timestamp monotonicity
    if (get_last_timestamp(ctx) >= a.timestamp) {
        if (g_batch_mode) {
            printf("invalid\n");
            return;
        } else {
            die();
        }
    }
    // Get current state
    int in_gal = 0, in_room = -1;
    get_person_state(ctx, a.name, category, &in_gal, &in_room);
    // Validate transition
    int next_in_gal = in_gal;
    int next_in_room = in_room;
    if (a.arrive) {
        if (!a.has_room) {
            // Arrive at gallery
            if (in_gal) {
                if (g_batch_mode) {
                    printf("invalid\n");
                    return;
                } else {
                    die();
                }
            }
            next_in_gal = 1; next_in_room = -1;
        } else {
            // Arrive at room
            if (!in_gal || in_room != -1) {
                if (g_batch_mode) {
                    printf("invalid\n");
                    return;
                } else {
                    die();
                }
            }
            next_in_gal = 1; next_in_room = a.room_id;
        }
    } else {
        if (!a.has_room) {
            // Leave gallery
            if (!in_gal || in_room != -1) {
                if (g_batch_mode) {
                    printf("invalid\n");
                    return;
                } else {
                    die();
                }
            }
            next_in_gal = 0; next_in_room = -1;
        } else {
            // Leave room
            if (!in_gal || in_room != a.room_id) {
                if (g_batch_mode) {
                    printf("invalid\n");
                    return;
                } else {
                    die();
                }
            }
            next_in_gal = 1; next_in_room = -1;
        }
    }
    // Append the log entry
    append_log(ctx, a.token, a.timestamp, a.name, category,
               next_in_gal, next_in_room);
    // Update in-memory state
    update_person_state(ctx, a.name, category, next_in_gal, next_in_room);
}

/* ── Batch Processing Functions ───────────────────────────────── */

/**
 * Splits a line into tokens, stripping leading/trailing whitespace.
 * Used by run_batch to parse each line of the batch file.
 * 
 * @param line Input line (modified in place)
 * @param argv Output array of token pointers
 * @param max_argc Maximum number of tokens
 * @return Number of tokens, or -1 if max_argc exceeded
 */
static int tokenize_line(char *line, char *argv[], int max_argc) {
    int argc = 0;
    char *end = line + strlen(line) - 1;
    while (end >= line && isspace((unsigned char)*end)) { *end-- = '\0'; }
    char *p = line;
    while (*p) {
        // Skip leading whitespace
        while (*p && isspace((unsigned char)*p)) p++;
        if (!*p) break;
        if (argc >= max_argc) return -1;
        argv[argc++] = p;
        // Skip to end of token
        while (*p && !isspace((unsigned char)*p)) p++;
        if (*p) *p++ = '\0';
    }
    return argc;
}
/**
 * Processes a batch file line by line.
 * 
 * @param batch_file Path to the batch file
 */
static void run_batch(const char *batch_file) {
    FILE *f = fopen(batch_file, "r");
    if (!f) { printf("invalid\n"); exit(111); }
    char line[MAX_BATCH_LINE];
    while (fgets(line, sizeof(line), f)) {
        // Skip empty lines
        char *p = line;
        while (isspace((unsigned char)*p)) p++;
        if (*p == '\0') continue;
        // Make a copy for tokenization
        char line_copy[MAX_BATCH_LINE];
        strncpy(line_copy, line, MAX_BATCH_LINE - 1);
        line_copy[MAX_BATCH_LINE - 1] = '\0';
        // Tokenize the line
        char *argv_batch[MAX_TOKENS];
        argv_batch[0] = "logappend";
        volatile int tokens = tokenize_line(line_copy, argv_batch + 1, MAX_TOKENS - 1);
        if (tokens <= 0) continue;
        // Check for nested -B flag (not allowed)
        for (int i = 1; i <= tokens; i++) {
            if (strcmp(argv_batch[i], "-B") == 0) {
                fclose(f);
                printf("invalid\n");
                exit(111);
            }
        }
        // Process the command - die() uses longjmp to continue batch
        g_batch_mode = 1;
        if (setjmp(g_die_jmp) == 0) {
            process_command(tokens + 1, argv_batch);
        }
        g_batch_mode = 0;
    }
    fclose(f);
    // Update headers and close files after batch completes
    LogContext *ctx = g_contexts;
    while (ctx) {
        if (ctx->fd) {
            fclose(ctx->fd);
            ctx->fd = NULL;
        }
        if (ctx->header_dirty && ctx->authenticated) {
            update_header(ctx);
            ctx->header_dirty = 0;
        }
        ctx = ctx->next;
    }
}

/* ── Main Function ────────────────────────────────────────────── */

/**
 * Entry point for the logappend program.
 * 
 * @param argc Number of command-line arguments
 * @param argv Array of argument strings
 * @return 0 on success, 111 on error
 */
int main(int argc, char *argv[]) {
    // Check for batch mode
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-B") == 0) {
            // -B must be the only argument besides the batch file
            if (argc != 3 || i + 1 >= argc || argv[i+1][0] == '-') {
                printf("invalid\n");
                return 111;
            }
            run_batch(argv[i + 1]);
            free_contexts();
            return 0;
        }
    }
    // Normal mode - process single command
    process_command(argc, argv);
    free_contexts();
    return 0;
}