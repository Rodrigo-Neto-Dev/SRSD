# Variáveis de Compilação
CC = gcc
CFLAGS = -Wall -Wextra -O2
LIBS = -lssl -lcrypto

# Alvos principais (os executáveis)
TARGETS = logappend logread

# Regra padrão: compila tudo
all: $(TARGETS)

# Compilação do logappend
logappend: logappend.c
	$(CC) $(CFLAGS) -o logappend logappend.c $(LIBS)

# Compilação do logread
logread: logread.c
	$(CC) $(CFLAGS) -o logread logread.c $(LIBS)

# Limpeza dos ficheiros binários
clean:
	rm -f $(TARGETS)

# Indica que estes alvos não são ficheiros
.PHONY: all clean