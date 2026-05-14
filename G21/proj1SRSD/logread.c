#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <limits.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <openssl/hmac.h>

#define MAX_LINE 1024
#define MAX_NAME 256
#define INITIAL_PEOPLE 100     
#define INITIAL_ROOMS 100    
#define GROWTH_FACTOR 2       
#define MAX_ALLOC_SIZE (1024 * 1024 * 100) 

/* Encryption constants */
#define SALT_LEN 16
#define HEADER_IV_LEN 12
#define HEADER_TAG_LEN 16
#define ENTRY_IV_LEN 12
#define ENTRY_TAG_LEN 16
#define HEADER_CIPHER_LEN 32

typedef struct {
    int room_id;        
    int enter_time;     
    int leave_time;    
} RoomInterval;

typedef struct {
    char name[MAX_NAME];               
    int is_employee;
    int in_gallery;                     
    int current_room;
    RoomInterval *room_history;     
    int history_count;                  
    int history_capacity;               
} Person;

typedef struct {
    unsigned char salt[SALT_LEN];  
} HeaderPlain;

typedef struct {
    uint64_t total_entries;    
    uint64_t last_timestamp;    
    uint8_t last_tag[16];      
} HeaderTextToCipher;

typedef struct {
    uint8_t iv[HEADER_IV_LEN];             
    uint8_t nentriestag[HEADER_CIPHER_LEN]; 
    uint8_t tag[HEADER_TAG_LEN];            
} HeaderBlock;

/**
 * Metadata for each encrypted log entry.
 * Layout: [seq(4) | prev_tag(16) | plaintext_len(4) | iv(12)] = 36 bytes.
 * The AAD for each entry is seq + prev_tag (20 bytes total).
 */
typedef struct {
    uint32_t seq;              
    unsigned char prev_tag[16];
    uint32_t plaintext_len;   
    unsigned char iv[12];     
} EntryMetadata;

//Global state - dynamically allocated 
Person *people = NULL;         
int person_count = 0;         
int person_capacity = 0;        
int last_timestamp = -1;
int memory_cleaned = 0;        

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
 * Ensures the people array has enough capacity for one more person.
 * Grows the array by GROWTH_FACTOR if needed.
 * 
 * @return 1 on success, 0 on failure (out of memory)
 */
static int ensure_people_capacity(void) {
    // Enough capacity already
    if (person_count < person_capacity) {
        return 1;
    }
    int new_capacity = (person_capacity == 0) ? INITIAL_PEOPLE : person_capacity * GROWTH_FACTOR;
    // Check for integer overflow
    if (new_capacity < person_capacity) {
        return 0;
    }
    // Check for allocation size overflow
    if (would_overflow(new_capacity, sizeof(Person))) {
        return 0;
    }
    // Check against maximum allocation size
    if (new_capacity * sizeof(Person) > MAX_ALLOC_SIZE) {
        return 0;
    }
    Person *new_people = realloc(people, new_capacity * sizeof(Person));
    if (!new_people) {
        return 0;
    }
    people = new_people;
    person_capacity = new_capacity;
    // Initialize new entries to zero
    memset(&people[person_count], 0, (person_capacity - person_count) * sizeof(Person));
    return 1;
}
/**
 * Ensures a person's room_history array has enough capacity for one more room.
 * Grows the array by GROWTH_FACTOR if needed.
 * 
 * @param p Index of the person in the people array
 * @return 1 on success, 0 on failure (out of memory)
 */
static int ensure_room_history_capacity(int p) {
    // Enough capacity already
    if (people[p].history_count < people[p].history_capacity) {
        return 1;
    }
    int new_capacity = (people[p].history_capacity == 0) ? INITIAL_ROOMS 
                                                         : people[p].history_capacity * GROWTH_FACTOR;
    
    // Check for integer overflow
    if (new_capacity < people[p].history_capacity) {
        return 0;
    }
    // Check for allocation size overflow
    if (would_overflow(new_capacity, sizeof(RoomInterval))) {
        return 0;
    }
    // Check against maximum allocation size
    if (new_capacity * sizeof(RoomInterval) > MAX_ALLOC_SIZE) {
        return 0;
    }
    RoomInterval *new_history = realloc(people[p].room_history, new_capacity * sizeof(RoomInterval));
    if (!new_history) {
        return 0;
    }
    people[p].room_history = new_history;
    people[p].history_capacity = new_capacity;
    return 1;
}
/**
 * Frees all dynamically allocated memory safely.
 * Uses OPENSSL_cleanse for sensitive data and prevents double free.
 */
static void free_all_memory(void) {
    // Prevent double free
    if (memory_cleaned) return;
    if (people) {
        for (int i = 0; i < person_count; i++) {
            if (people[i].room_history) {
                // Wipe sensitive data before freeing
                OPENSSL_cleanse(people[i].room_history, 
                                people[i].history_capacity * sizeof(RoomInterval));
                free(people[i].room_history);
                people[i].room_history = NULL;
            }
        }
        // Wipe people array before freeing
        OPENSSL_cleanse(people, person_capacity * sizeof(Person));
        free(people);
        people = NULL;
    }
    person_count = 0;
    person_capacity = 0;
    memory_cleaned = 1;
}

/* ── Crypto Functions ─────────────────────────────────────────── */

/**
 * Derives an AES-256 encryption key from token and salt.
 * Uses PBKDF2-HMAC-SHA256 with 100,000 iterations.
 * 
 * @param token User-provided authentication token (alphanumeric string)
 * @param salt 16-byte salt read from the log file
 * @param key Output buffer where the 32-byte derived key will be stored
 */
static void gen_key(const char *token, const unsigned char *salt, unsigned char *key) {
    static const unsigned char default_salt[16] = "FIXED_SALT_1234";
    if (!salt) salt = default_salt;
    if (!PKCS5_PBKDF2_HMAC(token, strlen(token),
                            salt, 16,
                            1, EVP_sha256(),
                            32, key)) {
        exit(111);
    }
}
/**
 * Decrypts data using AES-256-GCM with integrity verification.
 * The tag is verified during EVP_DecryptFinal_ex - if it fails,
 * 
 * @param ciphertext Encrypted data to decrypt
 * @param ciphertext_len Length of ciphertext
 * @param plaintext Output buffer for decrypted plaintext
 * @param plaintext_len Pointer to store actual plaintext length
 * @param key AES-256 key (32 bytes)
 * @param iv Initialization vector (12 bytes)
 * @param aad Additional Authenticated Data (can be NULL)
 * @param aad_len Length of AAD
 * @param tag GCM authentication tag (16 bytes) for verification
 * @return 1 on success (tag verified), 0 on failure (tag mismatch or error)
 */
static int decrypt_data(const unsigned char *ciphertext, size_t ciphertext_len,
                        unsigned char *plaintext, size_t *plaintext_len,
                        const unsigned char *key, const unsigned char *iv,
                        const unsigned char *aad, size_t aad_len,
                        const unsigned char *tag) {
    EVP_CIPHER_CTX *ctx = NULL;
    int len;
    // Parameter validation
    if (!ciphertext || !plaintext || !plaintext_len || !key || !iv || !tag)
        return 0;
    ctx = EVP_CIPHER_CTX_new();
    if (!ctx) return 0;
    // Initialize context with AES-256-GCM
    if (EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL) != 1)
        goto cleanup;
    // Set IV length to 12 bytes
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, ENTRY_IV_LEN, NULL) != 1)
        goto cleanup;
    // Set key and IV
    if (EVP_DecryptInit_ex(ctx, NULL, NULL, key, iv) != 1)
        goto cleanup;
    // Process AAD if present
    if (aad && aad_len > 0) {
        if (EVP_DecryptUpdate(ctx, NULL, &len, aad, aad_len) != 1)
            goto cleanup;
    }
    // Decrypt data
    if (EVP_DecryptUpdate(ctx, plaintext, &len, ciphertext, ciphertext_len) != 1)
        goto cleanup;
    *plaintext_len = len;
    // Set expected tag for verification
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, ENTRY_TAG_LEN, (void *)tag) != 1)
        goto cleanup;
    // Finalize and verify tag
    if (EVP_DecryptFinal_ex(ctx, plaintext + len, &len) != 1)
        goto cleanup;
    *plaintext_len += len;
    EVP_CIPHER_CTX_free(ctx);
    return 1;
cleanup:
    EVP_CIPHER_CTX_free(ctx);
    return 0;
}
/**
 * Validates that a file path contains only safe characters.
 * 
 * @param s File path to validate
 * @return 1 if path contains only safe characters, 0 otherwise
 */
static int is_valid_filepath(const char *s) {
    if (!s || *s == '\0') return 0;
    for (; *s; s++) {
        unsigned char c = (unsigned char)*s;
        if (!isalnum(c) && c != '_' && c != '.' && c != '/') return 0;
    }
    return 1;
}

/* ── Log Processing Functions ─────────────────────────────────── */

/**
 * Finds a person by name and type in the global people array.
 * Employees and guests can have the same name, so both fields are compared.
 * 
 * @param name Person's name to search for
 * @param is_employee 1 for employee, 0 for guest
 * @return Index in people array if found, -1 if not found
 */
int find_person(char *name, int is_employee) {
    for (int i = 0; i < person_count; i++) {
        if (strcmp(people[i].name, name) == 0 && people[i].is_employee == is_employee) {
            return i;
        }
    }
    return -1;
}
/**
 * Adds a new person to the global people array.
 * Uses dynamic memory allocation - grows array as needed.
 * 
 * @param name Person's name
 * @param is_employee 1 for employee, 0 for guest
 * @return Index of newly added person, or -1 on memory allocation failure
 */
int add_person(char *name, int is_employee) {
    // Ensure enough capacity for new person
    if (!ensure_people_capacity()) {
        return -1;
    }
    int idx = person_count;
    strncpy(people[idx].name, name, MAX_NAME - 1);
    people[idx].name[MAX_NAME - 1] = '\0';
    people[idx].is_employee = is_employee;
    people[idx].in_gallery = 0;
    people[idx].current_room = -1;
    people[idx].room_history = NULL;
    people[idx].history_count = 0;
    people[idx].history_capacity = 0;
    person_count++;
    return idx;
}
/**
 * Parses a log line and updates the in-memory gallery state.
 * 
 * Format: timestamp|name|category|in_gallery|room_id
 * 
 * @param line Decrypted log line (without newline)
 * @return 1 on success, 0 on error (invalid format or timestamp order)
 */
int process_line(char *line) {
    char *parts[5];
    int i = 0;
    char line_copy[MAX_LINE];
    strncpy(line_copy, line, MAX_LINE - 1);
    line_copy[MAX_LINE - 1] = '\0';
    char *part = strtok(line_copy, "|");
    while (part && i < 5) {
        parts[i++] = part;
        part = strtok(NULL, "|");
    }
    if (i < 5) return 0;
    long ts = strtol(parts[0], NULL, 10);
    if (ts < 0 || ts > 1073741823L) return 0;
    int timestamp = (int)ts;
    // Check timestamp order
    if (timestamp <= last_timestamp) {
        return 0;
    }
    last_timestamp = timestamp;
    char *name = parts[1];
    char *type = parts[2];
    int is_employee = (strcmp(type, "employee") == 0);
    int in_gallery = atoi(parts[3]);
    // Room Id Validation
    long rid = strtol(parts[4], NULL, 10);
    if (rid < -1 || rid > 1073741823L) return 0;
    int room_id = (int)rid;
    // Find or create person
    int p = find_person(name, is_employee);
    if (p == -1) {
        p = add_person(name, is_employee);
        if (p == -1) return 0;  // Memory allocation failure
    }
    people[p].in_gallery = in_gallery;
    int prev_room = people[p].current_room;
    // Track room entry/exit with dynamic bounds checking
    if (room_id != -1 && prev_room != room_id) {
        // Ensure enough capacity for new room history entry
        if (!ensure_room_history_capacity(p)) {
            return 0;
        }
        people[p].room_history[people[p].history_count].room_id = room_id;
        people[p].room_history[people[p].history_count].enter_time = timestamp;
        people[p].room_history[people[p].history_count].leave_time = -1;
        people[p].history_count++;
    } else if (room_id == -1 && prev_room != -1) {
        for (int j = people[p].history_count - 1; j >= 0; j--) {
            if (people[p].room_history[j].leave_time == -1) {
                people[p].room_history[j].leave_time = timestamp;
                break;
            }
        }
    }
    people[p].current_room = room_id;
    return 1;
}

/* ── Query Functions ──────────────────────────────────────────── */

/**
 * Prints the current gallery state (-S mode).
 * 
 * Output format:
 *   Line 1: Comma-separated list of employees in the gallery (alphabetical)
 *   Line 2: Comma-separated list of guests in the gallery (alphabetical)
 *   Line N: For each occupied room: "room_id:name1,name2,..." (alphabetical)
 * 
 * Rooms are printed in ascending order, names in lexicographic order.
 */
void print_state(void) {
    char **employee_names = NULL;
    int employee_count = 0;
    // First pass: count employees in gallery
    for (int i = 0; i < person_count; i++) {
        if (people[i].in_gallery == 1 && people[i].is_employee == 1) {
            employee_count++;
        }
    }
    // Allocate array for employee names
    if (employee_count > 0) {
        employee_names = malloc(employee_count * sizeof(char *));
        if (!employee_names) {
            return;
        }
        int idx = 0;
        for (int i = 0; i < person_count; i++) {
            if (people[i].in_gallery == 1 && people[i].is_employee == 1) {
                employee_names[idx++] = people[i].name;
            }
        }
        // Sort alphabetically
        for (int i = 0; i < employee_count - 1; i++) {
            for (int j = i + 1; j < employee_count; j++) {
                if (strcmp(employee_names[i], employee_names[j]) > 0) {
                    char *temp = employee_names[i];
                    employee_names[i] = employee_names[j];
                    employee_names[j] = temp;
                }
            }
        }
        for (int i = 0; i < employee_count; i++) {
            printf("%s%s", employee_names[i], (i < employee_count - 1) ? "," : "");
        }
        free(employee_names);
        employee_names = NULL;
    }
    printf("\n");
    // Print guests in gallery
    char **guest_names = NULL;
    int guest_count = 0;
    for (int i = 0; i < person_count; i++) {
        if (people[i].in_gallery == 1 && people[i].is_employee == 0) {
            guest_count++;
        }
    }
    if (guest_count > 0) {
        guest_names = malloc(guest_count * sizeof(char *));
        if (!guest_names) {
            return;
        }
        int idx = 0;
        for (int i = 0; i < person_count; i++) {
            if (people[i].in_gallery == 1 && people[i].is_employee == 0) {
                guest_names[idx++] = people[i].name;
            }
        }
        for (int i = 0; i < guest_count - 1; i++) {
            for (int j = i + 1; j < guest_count; j++) {
                if (strcmp(guest_names[i], guest_names[j]) > 0) {
                    char *temp = guest_names[i];
                    guest_names[i] = guest_names[j];
                    guest_names[j] = temp;
                }
            }
        }
        for (int i = 0; i < guest_count; i++) {
            printf("%s%s", guest_names[i], (i < guest_count - 1) ? "," : "");
        }
        free(guest_names);
        guest_names = NULL;
    }
    printf("\n");
    // Find all rooms that have people
    int *rooms = NULL;
    int rooms_count = 0;
    for (int i = 0; i < person_count; i++) {
        if (people[i].in_gallery == 1 && people[i].current_room > -1) {
            int room = people[i].current_room;
            int exists = 0;
            for (int j = 0; j < rooms_count; j++) {
                if (rooms[j] == room) {
                    exists = 1;
                    break;
                }
            }
            if (!exists) {
                int *new_rooms = realloc(rooms, (rooms_count + 1) * sizeof(int));
                if (!new_rooms) {
                    free(rooms);
                    return;
                }
                rooms = new_rooms;
                rooms[rooms_count++] = room;
            }
        }
    }
    // Sort rooms
    for (int i = 0; i < rooms_count - 1; i++) {
        for (int j = i + 1; j < rooms_count; j++) {
            if (rooms[i] > rooms[j]) {
                int temp = rooms[i];
                rooms[i] = rooms[j];
                rooms[j] = temp;
            }
        }
    }
    for (int s = 0; s < rooms_count; s++) {
        int room_id = rooms[s];
        char **names = NULL;
        int names_count = 0;
        // Count people in this room
        for (int i = 0; i < person_count; i++) {
            if (people[i].in_gallery == 1 && people[i].current_room == room_id) {
                names_count++;
            }
        }
        if (names_count > 0) {
            names = malloc(names_count * sizeof(char *));
            if (!names) {
                free(rooms);
                return;
            }
            int idx = 0;
            for (int i = 0; i < person_count; i++) {
                if (people[i].in_gallery == 1 && people[i].current_room == room_id) {
                    names[idx++] = people[i].name;
                }
            }
            for (int i = 0; i < names_count - 1; i++) {
                for (int j = i + 1; j < names_count; j++) {
                    if (strcmp(names[i], names[j]) > 0) {
                        char *temp = names[i];
                        names[i] = names[j];
                        names[j] = temp;
                    }
                }
            }
            printf("%d:", room_id);
            for (int i = 0; i < names_count; i++) {
                printf("%s%s", names[i], (i < names_count - 1) ? "," : "");
            }
            printf("\n");
            free(names);
            names = NULL;
        }
    }
    free(rooms);
    rooms = NULL;
}
/**
 * Prints all rooms visited by a specific person (-R mode).
 * Output is a comma-separated list of room IDs in chronological order.
 * If the person doesn't exist in the log, nothing is printed.
 * 
 * @param name Person's name to query
 * @param is_employee 1 for employee, 0 for guest
 */
void print_rooms_visited(char *name, int is_employee) {
    int p = find_person(name, is_employee);
    if (p != -1) {
        for (int i = 0; i < people[p].history_count; i++) {
            printf("%d%s", people[p].room_history[i].room_id, 
                   (i < people[p].history_count - 1) ? "," : "");
        }
        printf("\n");
    }
}
/**
 * Checks if two time intervals overlap.
 * 
 * @param a First interval
 * @param b Second interval
 * @return 1 if intervals overlap, 0 otherwise
 */
int intervals_overlap(RoomInterval *a, RoomInterval *b) {
    int a_leave = (a->leave_time == -1) ? INT_MAX : a->leave_time;
    int b_leave = (b->leave_time == -1) ? INT_MAX : b->leave_time;
    return a->enter_time < b_leave && b->enter_time < a_leave;
}
/**
 * Checks if all specified people were simultaneously in a room.
 * 
 * @param room Room ID to check
 * @param person_indices_map Array mapping query indices to people array indices
 * @param count Number of people in the query
 * @return 1 if all were simultaneously in the room, 0 otherwise
 */
int check_simultaneous(int room, int *person_indices_map, int count) {
    int first_idx = -1;
    for (int p = 0; p < count; p++) {
        if (person_indices_map[p] != -1) { first_idx = p; break; }
    }
    if (first_idx == -1) return 0;
    int idx0 = person_indices_map[first_idx];
    // Try each interval of the first person
    for (int i = 0; i < people[idx0].history_count; i++) {
        if (people[idx0].room_history[i].room_id != room) continue;
        int enter0 = people[idx0].room_history[i].enter_time;
        int leave0 = (people[idx0].room_history[i].leave_time == -1)
                     ? INT_MAX : people[idx0].room_history[i].leave_time;
        int window_enter = enter0;
        int window_leave = leave0;
        int all_overlap = 1;
        for (int p = 0; p < count; p++) {
            if (p == first_idx) continue;
            int idx = person_indices_map[p];
            if (idx == -1) continue;
            int found = 0;
            for (int j = 0; j < people[idx].history_count; j++) {
                if (people[idx].room_history[j].room_id != room) continue;

                int enter_j = people[idx].room_history[j].enter_time;
                int leave_j = (people[idx].room_history[j].leave_time == -1)
                              ? INT_MAX : people[idx].room_history[j].leave_time;

                if (enter_j < window_leave && leave_j > window_enter) {
                    // Narrow the window to the overlap
                    if (enter_j > window_enter) window_enter = enter_j;
                    if (leave_j < window_leave) window_leave = leave_j;
                    found = 1;
                    break;
                }
            }
            if (!found) { all_overlap = 0; break; }
        }
        if (all_overlap && window_enter < window_leave) return 1;
    }
    return 0;
}
/**
 * Prints rooms where all specified people were together (-I mode).
 * Output is a comma-separated list of room IDs in ascending order.
 * Persons that don't exist in the log are ignored.
 * If no room contained all specified persons simultaneously, nothing is printed.
 * 
 * @param names Array of person names
 * @param is_employee Array of person types (1=employee, 0=guest)
 * @param count Number of persons in the query
 */
void print_intersection(char **names, int *is_employee, int count) {
        // Check for zero count before allocation
    if (count == 0) return;
    // Map each name to person index
    int *person_indices_map = malloc(count * sizeof(int));
    if (!person_indices_map) {
        return;
    }
    int found_count = 0;
    for (int p = 0; p < count; p++) {
        int person_idx = find_person(names[p], is_employee[p]);
        if (person_idx == -1) {
            person_indices_map[p] = -1;
        } else {
            person_indices_map[p] = person_idx;
            found_count++;
        }
    }
    if (found_count == 0) {
        free(person_indices_map);
        person_indices_map = NULL;
        return;
    }
    // Collect all rooms visited
    int *all_rooms = NULL;
    int all_rooms_count = 0;
    for (int p = 0; p < count; p++) {
        if (person_indices_map[p] != -1) {
            int person_idx = person_indices_map[p];
            for (int i = 0; i < people[person_idx].history_count; i++) {
                int room = people[person_idx].room_history[i].room_id;
                int found = 0;
                for (int k = 0; k < all_rooms_count; k++) {
                    if (all_rooms[k] == room) {
                        found = 1;
                        break;
                    }
                }
                if (!found) {
                    int *new_rooms = realloc(all_rooms, (all_rooms_count + 1) * sizeof(int));
                    if (!new_rooms) {
                        free(all_rooms);
                        free(person_indices_map);
                        return;
                    }
                    all_rooms = new_rooms;
                    all_rooms[all_rooms_count++] = room;
                }
            }
        }
    }
    // Special case: only one person - just list their unique rooms
    if (found_count == 1) {
        int person_idx = -1;
        for (int p = 0; p < count; p++) {
            if (person_indices_map[p] != -1) {
                person_idx = person_indices_map[p];
                break;
            }
        }
        int *unique_rooms = NULL;
        int unique_count = 0;
        for (int i = 0; i < people[person_idx].history_count; i++) {
            int room = people[person_idx].room_history[i].room_id;
            if (i == 0 || people[person_idx].room_history[i].room_id != people[person_idx].room_history[i-1].room_id) {
                int *new_unique = realloc(unique_rooms, (unique_count + 1) * sizeof(int));
                if (!new_unique) {
                    free(unique_rooms);
                    free(all_rooms);
                    free(person_indices_map);
                    return;
                }
                unique_rooms = new_unique;
                unique_rooms[unique_count++] = room;
            }
        }
        // Sort rooms
        for (int i = 0; i < unique_count - 1; i++) {
            for (int j = i + 1; j < unique_count; j++) {
                if (unique_rooms[i] > unique_rooms[j]) {
                    int temp = unique_rooms[i];
                    unique_rooms[i] = unique_rooms[j];
                    unique_rooms[j] = temp;
                }
            }
        }
        // Remove duplicates and print
        for (int i = 0; i < unique_count; i++) {
            if (i == 0 || unique_rooms[i] != unique_rooms[i-1]) {
                printf("%d%s", unique_rooms[i], (i < unique_count - 1) ? "," : "");
            }
        }
        printf("\n");
        free(unique_rooms);
        unique_rooms = NULL;
        free(all_rooms);
        all_rooms = NULL;
        free(person_indices_map);
        person_indices_map = NULL;
        return;
    }
    // Multiple people: find common rooms
    int *common_rooms = NULL;
    int common_count = 0;
    for (int r = 0; r < all_rooms_count; r++) {
        int room = all_rooms[r];
        if (check_simultaneous(room, person_indices_map, count)) {
            int *new_common = realloc(common_rooms, (common_count + 1) * sizeof(int));
            if (!new_common) {
                free(common_rooms);
                free(all_rooms);
                free(person_indices_map);
                return;
            }
            common_rooms = new_common;
            common_rooms[common_count++] = room;
        }
    }
    // Sort rooms
    for (int i = 0; i < common_count - 1; i++) {
        for (int j = i + 1; j < common_count; j++) {
            if (common_rooms[i] > common_rooms[j]) {
                int temp = common_rooms[i];
                common_rooms[i] = common_rooms[j];
                common_rooms[j] = temp;
            }
        }
    }
    for (int i = 0; i < common_count; i++) {
        printf("%d%s", common_rooms[i], (i < common_count - 1) ? "," : "");
    }
    printf("\n");
    free(common_rooms);
    common_rooms = NULL;
    free(all_rooms);
    all_rooms = NULL;
    free(person_indices_map);
    person_indices_map = NULL;
}

/* ==================== MAIN FUNCTION ==================== */

/**
 * Main entry point for the logread program.
 * 
 * @param argc Number of command-line arguments
 * @param argv Array of argument strings
 * @return 0 on success, 111 on error
 */
int main(int argc, char *argv[]) {
    setbuf(stdout, NULL);
    char *token = NULL;
    char *logfile = NULL;
    char *mode = NULL;
    char *query_name = NULL;
    int query_is_employee = 0;
    // For -I mode - dynamically allocated
    char **names = NULL;
    int *is_employee_arr = NULL;
    int name_count = 0;
    int name_capacity = 0;  /* Track allocated capacity for -I mode */
    
    int has_K = 0, has_mode = 0, has_query_name = 0, has_logfile = 0;
    // First pass: count arguments to allocate properly for -I mode
    int i = 1;
    while (i < argc) {
        if (strcmp(argv[i], "-I") == 0) {
            i++;
            // Count how many -E and -G follow
            while (i < argc && (strcmp(argv[i], "-E") == 0 || strcmp(argv[i], "-G") == 0)) {
                if (i + 1 < argc && argv[i+1][0] != '-') {
                    name_count++;
                    i += 2;
                } else {
                    break;
                }
            }
            break;
        }
        i++;
    }
    // Allocate arrays for -I mode if needed
    if (name_count > 0) {
        names = malloc(name_count * sizeof(char *));
        is_employee_arr = malloc(name_count * sizeof(int));
        if (!names || !is_employee_arr) {
            printf("invalid\n");
            free(names);
            free(is_employee_arr);
            return 111;
        }
        name_capacity = name_count;  /* Save capacity */
        name_count = 0;  /* Reset to fill during second pass */
    }
    // Reset and parse command line arguments properly
    i = 1;
    has_K = 0; has_mode = 0; has_query_name = 0; has_logfile = 0;
    for (i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-K") == 0 && i+1 < argc) {
            if (has_K) { printf("invalid\n"); goto cleanup_names; }
            token = argv[++i];
            has_K = 1;
        }
        else if (strcmp(argv[i], "-S") == 0) {
            if (has_mode) { printf("invalid\n"); goto cleanup_names; }
            mode = "S";
            has_mode = 1;
        }
        else if (strcmp(argv[i], "-R") == 0) {
            if (has_mode) { printf("invalid\n"); goto cleanup_names; }
            mode = "R";
            has_mode = 1;
        }
        else if (strcmp(argv[i], "-I") == 0) {
            if (has_mode) { printf("invalid\n"); goto cleanup_names; }
            mode = "I";
            has_mode = 1;
        }
        else if (strcmp(argv[i], "-E") == 0 && i+1 < argc) {
            if (mode && strcmp(mode, "I") == 0) {
                if (names && is_employee_arr && name_count < name_capacity) {
                    names[name_count] = argv[++i];
                    is_employee_arr[name_count] = 1;
                    name_count++;
                } else {
                    i++;
                }
            } else {
                if (has_query_name) { printf("invalid\n"); goto cleanup_names; }
                query_name = argv[++i];
                query_is_employee = 1;
                has_query_name = 1;
            }
        }
        else if (strcmp(argv[i], "-G") == 0 && i+1 < argc) {
            if (mode && strcmp(mode, "I") == 0) {
                if (names && is_employee_arr && name_count < name_capacity) {
                    names[name_count] = argv[++i];
                    is_employee_arr[name_count] = 0;
                    name_count++;
                } else {
                    i++;
                }
            } else {
                if (has_query_name) { printf("invalid\n"); goto cleanup_names; }
                query_name = argv[++i];
                query_is_employee = 0;
                has_query_name = 1;
            }
        }
        else if (argv[i][0] != '-') {
            if (has_logfile || !is_valid_filepath(argv[i])) {
                printf("invalid\n");
                goto cleanup_names;
            }
            logfile = argv[i];
            has_logfile = 1;
        }
    }
    // Validate required arguments
    if (!token || !logfile || !mode) {
        printf("invalid\n");
        goto cleanup_names;
    }
    if (strcmp(mode, "R") == 0 && !query_name) {
        printf("invalid\n");
        goto cleanup_names;
    }
    // Open file, read salt
    FILE *fp = fopen(logfile, "rb");
    if (!fp) {
        printf("integrity violation\n");
        goto cleanup_names;
    }
    // Read salt (HeaderPlain) - 16 bytes in plaintext at beginning of file
    HeaderPlain hp;
    if (fread(&hp, sizeof(HeaderPlain), 1, fp) != 1) {
        fclose(fp);
        printf("integrity violation\n");
        goto cleanup_names;
    }
    // Derive key from token and salt read from file
    unsigned char key[32];
    gen_key(token, hp.salt, key);
    // Read and decrypt Header
    HeaderBlock header;
    if (fread(&header, sizeof(HeaderBlock), 1, fp) != 1) {
        fclose(fp);
        printf("integrity violation\n");
        goto cleanup_names;
    }
    // Decrypt header with salt as AAD - compatible with logappend
    HeaderTextToCipher htc;
    size_t decrypted_len;
    if (!decrypt_data(header.nentriestag, HEADER_CIPHER_LEN,
                      (unsigned char *)&htc, &decrypted_len,
                      key, header.iv,
                      (unsigned char *)&hp, sizeof(HeaderPlain),  // AAD = salt
                      header.tag)) {
        fclose(fp);
        printf("integrity violation\n");
        goto cleanup_names;
    }
    if (decrypted_len != sizeof(HeaderTextToCipher)) {
        fclose(fp);
        printf("integrity violation\n");
        goto cleanup_names;
    }
    uint64_t total_entries = htc.total_entries;
    // For the first entry, expected prev_tag is all zeros
    unsigned char expected_prev_tag[16] = {0};
    // Read and decrypt each entry
    uint32_t expected_seq = 1;
    for (uint64_t j = 0; j < total_entries; j++) {
        EntryMetadata meta;
        // Read metadata (36 bytes)
        if (fread(&meta, sizeof(EntryMetadata), 1, fp) != 1) {
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // Verify sequence number
        if (meta.seq != expected_seq) {
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // Verify tag chain (prev_tag must match previous entry's tag)
        if (memcmp(meta.prev_tag, expected_prev_tag, 16) != 0) {
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // CHECK plaintext_len before malloc
        if (meta.plaintext_len == 0 || meta.plaintext_len > MAX_LINE) {
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // Read ciphertext
        unsigned char *ciphertext = malloc(meta.plaintext_len);
        if (!ciphertext) {
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        if (fread(ciphertext, 1, meta.plaintext_len, fp) != meta.plaintext_len) {
            free(ciphertext);
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // Read tag (16 bytes)
        unsigned char tag[ENTRY_TAG_LEN];
        if (fread(tag, 1, ENTRY_TAG_LEN, fp) != ENTRY_TAG_LEN) {
            free(ciphertext);
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        // Prepare AAD: seq (4 bytes) + prev_tag (16 bytes) = 20 bytes
        unsigned char aad[20];
        memcpy(aad, &meta.seq, 4);
        memcpy(aad + 4, meta.prev_tag, 16);
        // Decrypt the entry
        unsigned char plaintext[MAX_LINE];
        size_t plaintext_len;
        if (!decrypt_data(ciphertext, meta.plaintext_len,
                          plaintext, &plaintext_len,
                          key, meta.iv,
                          aad, 20,
                          tag)) {
            free(ciphertext);
            fclose(fp);
            printf("integrity violation\n");
            goto cleanup_names;
        }
        free(ciphertext);
        // Null-terminate and remove newline
        plaintext[plaintext_len] = '\0';
        char *line_str = (char *)plaintext;
        line_str[strcspn(line_str, "\n")] = '\0';
        // Process the decrypted line
        if (strlen(line_str) > 0) {
            if (!process_line(line_str)) {
                fclose(fp);
                printf("invalid\n");
                goto cleanup_names;
            }
        }
        // Update for next entry
        memcpy(expected_prev_tag, tag, 16);
        expected_seq++;
    }
    fclose(fp);
    // Execute requested query
    if (strcmp(mode, "S") == 0) {
        print_state();
    } else if (strcmp(mode, "R") == 0) {
        print_rooms_visited(query_name, query_is_employee);
    } else if (strcmp(mode, "I") == 0) {
        print_intersection(names, is_employee_arr, name_count);
    }
    // Free all dynamically allocated memory before exiting
    free_all_memory();
    
cleanup_names:
    free(names);
    free(is_employee_arr);
    // Wipe sensitive key from memory
    OPENSSL_cleanse(key, sizeof(key));
    return 0;
}