# Gallery Log - Segurança de Redes e Sistemas Distribuídos

Este projeto implementa um sistema seguro de registo (log) para controlo de entradas e saídas numa galeria de arte.

## Execução

O sistema é composto por duas ferramentas principais: `logappend` (para inserir dados) e `logread` (para consultar dados).

O sistema está preparado para correr na VM da cadeira. Logo, as instruções apresentadas em baixo, servem para executar os comandos na VM da cadeira. Os comandos apresentados em baixo são feitos pelo terminal, na pasta raiz do projeto.
### 1. Adicionar Eventos (logappend)
Permite adicionar novas entradas e saídas ao log. Se o ficheiro não existir, será criado com o token fornecido e inicializado com o cabeçalho criptográfico.

**Sintaxe base:**
`./logappend -T <timestamp> -K <token> (-E <nome-empregado> | -G <nome-convidado>) (-A | -L) [-R <id-quarto>] <log_file>`

**Exemplos Práticos:**
* **Exemplo de Chegada (Galeria):** `./logappend -T 1 -K segredo -A -E Alice galeria.log`
* **Exemplo de Chegada (Quarto):** `./logappend -T 2 -K segredo -A -E Alice -R 5 galeria.log`

**Modo Batch:**
Permite executar múltiplos comandos sequenciais a partir de um ficheiro de texto.
`./logappend -B <ficheiro_batch>`

### 2. Consultar Eventos (logread)
Permite consultar o estado da galeria, verificar o histórico de uma pessoa ou descobrir em que salas várias pessoas se cruzaram. O comando abortará imediatamente se o ficheiro estiver corrompido, truncado, ou se o token não for o correto.

**Estado Atual da Galeria (-S):**
Imprime quem está atualmente na galeria e distribui as pessoas pelas respetivas salas.
`./logread -K <token> -S <log_file>`

**Histórico de Quartos (-R):**
Imprime a lista cronológica (separada por vírgulas) dos quartos visitados por uma pessoa específica.
`./logread -K <token> -R (-E <nome> | -G <nome>) <log_file>`

**Interseção de Pessoas (-I):** (Funcionalidade Opcional Implementada)
Verifica as salas onde as pessoas especificadas estiveram fisicamente juntas em simultâneo.
`./logread -K <token> -I -E <nome1> -G <nome2> <log_file>`

---

## Códigos de Saída (Exit Codes)

* **`0`**: Sucesso.
* **`111`**: Erro de execução. Este código é devolvido em diversas situações de falha, tais como: parâmetros inconsistentes, lógica de negócio inválida (ex: sair sem entrar), falha na validação da cadeia de blocos (Hash Chain), ou erro na decifragem AES-GCM (indicando *Integrity Violation*).