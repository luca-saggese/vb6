# VB6R — MX-SYNTAX

**Stato:** `DESIGN-APPROVED / IMPLEMENTATION-DEFERRED`
**Tipo:** specifica di progettazione per estensioni future del linguaggio
**Versione documento:** 0.1 — 13 ottobre 2026
**Ambito:** sintassi VB6R, frontend, analisi semantica, IR, runtime e integrazione VM
**Fuori ambito:** implementazione immediata, nuovi opcode Microsoft, ridefinizione delle milestone M1–M6

> **Decisione owner:** conservare questa specifica per un successivo ciclo MX. Le sedici estensioni elencate sono approvate come *direzioni progettuali*, non come grammatica o semantica già congelata. Gli esempi contrassegnati **PROPOSTA** non attestano funzionalità implementate.

## 1. Obiettivo e vincoli non negoziabili

VB6R deve preservare il comportamento dei programmi Visual Basic 6 originali e offrire, in modalità esplicita, estensioni moderne che conservino la leggibilità e lo stile del linguaggio. Non si intende trasformare VB6R in VB.NET.

1. **Compatibilità storica.** In modalità legacy, codice VB6 valido conserva parsing e semantica originali: `Variant` implicito, `On Error`, `GoSub/Return`, `And`/`Or`, `Null`, `Empty`, `Nothing`, `ByRef`, regole numeriche e stringhe.
2. **Opt-in esplicito.** Sintassi proposta per l'abilitazione: `Option VB6RExtensions On` a livello di modulo/progetto. Nome, posizione e default vanno ratificati nella progettazione del parser. In sua assenza, le nuove produzioni grammaticali non devono reinterpretare codice VB6 esistente.
3. **Livelli distinti.** Le estensioni si abbassano a **IR/bytecode interno VB6R versionato** e primitive runtime. Le sei tabelle P-Code Microsoft e i loro opcode **non si modificano**.
4. **Compatibilità cross-platform.** Evitare che la grammatica imponga Windows, COM o un toolkit GUI. Le API esterne e i servizi di piattaforma sono adattatori separati.
5. **Diagnostica.** Funzionalità non ancora supportate devono dare un errore esplicito di compilazione; mai essere interpretate come altro codice valido o abbassate a `Nop`.
6. **Modello asincrono MX già fissato.** `VBTask.Then` e `.Catch` registrano callback; alla loro invocazione la VM crea **un nuovo frame di procedura**. **Niente `Async/Await`, `AwaitTask`, frame sospesi o scheduler implicito di continuazioni**.

## 2. Repertorio concordato — 16 estensioni

| ID | Funzionalità | Priorità proposta | Responsabilità prevalente | Stato |
|---|---|---|---|---|
| MX-S01 | `Return <expr>` nelle `Function` | P0 | Parser → semantica → IR | Da progettare/implementare |
| MX-S02 | `Continue For`, `Continue Do` | P0 | Control-flow lowering | Da progettare/implementare |
| MX-S03 | `AndAlso`, `OrElse` | P0 | Short-circuit e controllo di flusso | Da progettare/implementare |
| MX-S04 | Inferenza esplicita: `As Auto` | P1 | Type inference locale | Da progettare/implementare |
| MX-S05 | Interpolazione stringhe `$"..."` | P1 | Parser e conversione stringhe VB6 | Da progettare/implementare |
| MX-S06 | `Try` / `Catch` / `Finally` | P1 | Error handling sincrono e cleanup | Da progettare/implementare |
| MX-S07 | `Using` | P1 | Protocollo di rilascio risorse | Da progettare/implementare |
| MX-S08 | `Option Strict On` | P1 | Diagnostica e conversioni implicite | Da progettare/implementare |
| MX-S09 | Accesso opzionale `?.` | P1 | Nullability e accesso a membri | Da progettare/implementare |
| MX-S10 | Coalescenza `??` | P1 | Nullability e lazy fallback | Da progettare/implementare |
| MX-S11 | Spread `...` | P1/P2 | Parametri, array e call lowering | Da progettare/implementare |
| MX-S12 | Letterali di array/collezione `[ ... ]` | P1/P2 | Sintassi, tipi, allocazione runtime | Da progettare/implementare |
| MX-S13 | Destructuring | P2 | Binding e assegnazione multipla | Da progettare/implementare |
| MX-S14 | Lambda e closure | P1, dopo i tipi procedura | Tipi funzione, capture, lifetime | Da progettare/implementare |
| MX-S15 | Generics `List(Of Long)` | P2 | Type system e specializzazione | Da progettare/implementare |
| MX-S16 | Pattern matching `Match` | P2 | Pattern typing e control-flow lowering | Da progettare/implementare |

Le priorità indicano *ordine di ingegneria suggerito*, non milestone già pianificate né obbligo di rilasciare tutte le funzioni contemporaneamente.

## 3. Sintassi proposta ed esiti attesi

Tutti gli esempi di questo capitolo sono **PROPOSTE DI SINTASSI FUTURA**, non codice già accettato dal compilatore attuale.

### MX-S01 — `Return <expr>`

```vb
Option VB6RExtensions On
Public Function Somma(ByVal a As Long, ByVal b As Long) As Long
    Return a + b
End Function
```

- Valuta l'espressione una sola volta, applica le conversioni di ritorno valide e termina la `Function`.
- Deve rispettare `Finally`/cleanup se presenti.
- **Conflitto VB6:** `Return` senza espressione appartiene a `GoSub/Return`. In modalità legacy resta invariato. In modalità estesa, `Return <expr>` è la nuova produzione; il significato di `Return` nudo **non viene modificato automaticamente**. La grammatica e la diagnostica devono gestire esplicitamente l'ambiguità.
- Nelle `Sub`, continuare a supportare `Exit Sub`; un eventuale `Return` senza valore come uscita da `Sub` è **non deciso**.

### MX-S02 — `Continue For` / `Continue Do`

```vb
For i = 1 To 10
    If i Mod 2 = 0 Then Continue For
    Print i
Next
```

- Trasferisce il controllo al punto di incremento/condizione corretto del ciclo attivo.
- `Continue Do` salta alla verifica di condizione prevista dalla specifica forma `Do/Loop`.
- Non deve saltare `Finally`/cleanup obbligatorio e non deve cambiare il significato di `Exit For`/`Exit Do`.

### MX-S03 — `AndAlso` / `OrElse`

```vb
If obj Is Not Nothing AndAlso obj.IsReady Then
    Print "Pronto"
End If
```

- Il secondo operando viene valutato **soltanto se necessario**.
- `And` e `Or` VB6 restano **immutati**, compresi gli usi bitwise e i comportamenti con `Null`/`Variant`.
- La conversione a Boolean e il trattamento di `Null` nell'operatore nuovo richiedono una regola esplicita prima dell'implementazione.

### MX-S04 — `As Auto`

```vb
Dim totale As Auto = 100
Dim messaggio As Auto = "Ciao"
```

- Il compilatore deduce il tipo statico dall'inizializzatore; l'inferenza non sostituisce le conversioni VB6.
- `Dim x` **continua a significare `Variant`** in VB6 compatibile.
- Requisito: inizializzatore obbligatorio, regole deterministiche per numerici/literal `Nothing` e diagnostica su tipo ambiguo.

### MX-S05 — Interpolazione stringhe

```vb
Dim messaggio As String
messaggio = $"Ciao {nome}, totale: {totale}!"
```

- Le espressioni interpolate vengono valutate una volta, nell'ordine previsto dal linguaggio.
- Conversione in stringa, escaping di `{`/`}` e virgolette, locale/cultura, `Null`, `Empty`, `Date` e `Currency` devono avere un contratto VB6R esplicito.
- Preferire il lowering verso concatenazione/runtime stringhe anziché nuove istruzioni Microsoft.

### MX-S06 — `Try` / `Catch` / `Finally`

```vb
Try
    CaricaDati
Catch ex As VBError
    LogError ex.Description
Finally
    RilasciaRisorse
End Try
```

- Gestisce **errori sincroni**, inclusi quelli provenienti dalle procedure VM; non è `VBTask.Catch`.
- Richiede gerarchia/tipo dell'errore, propagazione, unwinding, `Finally` su `Return`/`Exit`/errori e integrazione con `Err` e `On Error`.
- La precedenza tra `Try` e `On Error` in uno stesso frame è **questione aperta**, da specificare prima del rilascio.

### MX-S07 — `Using`

```vb
Using file = OpenTextFile("report.txt")
    file.WriteLine "Pronto"
End Using
```

- Garantisce il rilascio della risorsa all'uscita dall'ambito, anche su errore.
- Definire un protocollo esplicito (`Dispose`, `Close` o interfaccia VB6R dedicata); non presupporre che qualunque oggetto COM esponga un metodo equivalente.
- Il rilascio non deve sovrascrivere senza regole l'errore primario dell'ambito.

### MX-S08 — `Option Strict On`

```vb
Option VB6RExtensions On
Option Strict On
Dim totale As Long
totale = "123"    ' Errore di compilazione: conversione implicita non ammessa
```

- Impone controlli più restrittivi su narrowing, late binding e conversioni non sicure in modalità estesa.
- **Non cambia retroattivamente la semantica di un progetto VB6 legacy** né disabilita di nascosto operazioni storiche.
- Serve una matrice di conversioni consentite/vietate, inclusi `Variant`/`Null` e `ByRef`.

### MX-S09 — Accesso opzionale `?.`

```vb
citta = cliente?.Indirizzo?.Citta
```

- Se il ricevente è assente, non deve valutare l'accesso successivo.
- **Da definire:** quali stati attivano la propagazione opzionale (`Nothing`, `Null`, eventualmente `Empty`) e quale tipo/valore rappresenta l'assenza. Non sono sinonimi in VB6.
- Mai nascondere indiscriminatamente errori COM o accessi illegittimi dietro un valore predefinito.

### MX-S10 — Coalescenza `??`

```vb
citta = cliente?.Indirizzo?.Citta ?? "Non disponibile"
```

- Il secondo operando è valutato solo quando il primo è "assente" secondo una regola esplicita.
- Deve essere coerente con `?.`, ma non confondere automaticamente `Nothing`, `Null`, `Empty`, `0` e `""`.
- Definire precedenze, associatività e unificazione del tipo dei due operandi.

### MX-S11 — Spread `...`

```vb
numeri = [10, 20, 30]
risultato = Somma(...numeri)
```

- Espansione degli argomenti in una chiamata; per una funzione ad arità fissa il compilatore deve dimostrare o controllare che numero e tipi coincidano.
- Per arità dinamica, definire l'interazione con `ParamArray`, ordine di valutazione, temporanei, conversioni, `ByRef`, aliasing e ownership.
- Possibili usi futuri in letterali/collezioni **non sono automaticamente inclusi** nella prima implementazione.

### MX-S12 — Letterali di array/collezione

```vb
numeri = [10, 20, 30]
```

- Definire inferenza del tipo degli elementi, mutabilità, bounds, `Option Base`, empty literal e scelta tra array VB6 e collection VB6R.
- **Conflitto lessicale:** VB6 usa già `[Identificatore con spazi]` per nomi delimitati. La nuova grammatica deve essere non ambigua in modalità estesa e non reinterpretare sorgenti legacy.

### MX-S13 — Destructuring

```vb
Dim (x, y) = GetPoint()
```

- Introdurre contratti di prodotto (tuple/record/UDT) e compatibilità dei tipi.
- Valutazione della sorgente una sola volta; stabilire atomicità o comportamento delle assegnazioni parziali, inclusi casi `ByRef`.

### MX-S14 — Lambda e closure

```vb
Dim doppi As Auto = numeri.Map(Function(x) x * 2)
Button1.OnClick(Function()
    Label1.Caption = "Premuto!"
End Function)
```

- Definire tipi procedura/delegate, catture per valore o riferimento, lifetime, ricorsione, reentrancy e accesso ai controlli GUI.
- La VM invoca il corpo di una lambda tramite una nuova chiamata di procedura, non tramite frame sospesi.
- Le callback UI devono rispettare il thread GUI; evitare riferimenti pendenti a variabili locali catturate.
- Esempi di API `.Map`/`.OnClick` **illustrativi**, non un impegno ad aggiungerle implicitamente al runtime.

### MX-S15 — Generics

```vb
Dim valori As List(Of Long)
```

- Definire parametri di tipo, vincoli, specializzazione/erasure, compatibilità con classi VB6, `Variant` e COM.
- Le collezioni standard sono una libreria distinta; il costrutto sintattico non implica che `List` sia già implementata.

### MX-S16 — Pattern matching

```vb
Match valore
    Case 0
        Print "Zero"
    Case 1 To 9
        Print "Singola cifra"
    Case Else
        Print "Altro"
End Match
```

- Scegliere definitivamente sintassi e nomi delle produzioni, senza entrare in collisione con `Select Case` VB6.
- Definire verifica di esaustività, ordine dei pattern, binding, guardie e trattamento di `Variant`, `Null` e stringhe.

## 4. MX già deciso: `VBTask.Then` e `.Catch`

```vb
Option VB6RExtensions On

task.Then AddressOf OnCompleted
task.Catch AddressOf OnFailed
```

**Decisioni già fissate:**

- `Then` e `Catch` sono **metodi del runtime `VBTask`**, non parole chiave obbligatorie né opcode Microsoft nuovi.
- La registrazione della callback è distinta dalla sua esecuzione.
- Al completamento/fallimento viene invocata una **nuova procedura VM, con frame nuovo**.
- Nessun `Async`, `Await`, `AwaitTask`, stack frame sospeso o scheduler di ripresa.
- Le lambda, se implementate, potranno essere accettate come callback attraverso il normale modello dei valori funzione/closure.

**Da specificare nella milestone MX:** politica di consegna sul thread GUI, reentrancy, ordine delle callback, errore lanciato da una callback, cancellazione, durata dei task e ownership dei valori catturati. Non inferire questi aspetti dall'esempio.

## 5. Architettura di compilazione

```text
Sorgente VB6R con estensioni (opt-in)
          |
     Lexer / Parser
          |
       AST esteso
          |
 Binding / Type Checking / Flow Analysis
          |
 Lowering a IR VB6R tipizzato e versionato
          |
     VM + runtime M3/MX
          |
  Esecuzione / app M6 / packaging
```

Percorso indipendente e invariato:

```text
EXE originale Microsoft VB6
  -> Loader M2 -> decoder P-Code Microsoft -> handler VM -> runtime VB6R
```

Il compilatore può abbassare una nuova funzionalità a primitive/istruzioni IR esistenti. **Nuovi opcode interni VB6R** sono ammessi soltanto quando motivati, versionati e testati; **non** devono occupare o ridefinire slot delle tabelle Microsoft. Un futuro backend verso P-Code Microsoft può usare solo combinazioni di opcode Microsoft esistenti più chiamate runtime compatibili, nei limiti tecnicamente possibili.

## 6. Politica di compatibilità e di design

- **Legacy invariato:** `Dim x` mantiene il tipo `Variant`; `And`/`Or` non diventano short-circuit; `GoSub/Return` non viene reinterpretato; `On Error` continua a esistere.
- **Valori VB6 distinti:** `Empty`, `Null`, `Nothing`, `Missing` (dove applicabile), stringa vuota e zero richiedono semantiche e test separati.
- **Tipizzazione:** `Integer` (16 bit), `Long` (32 bit), `Currency`, `Date`, `Boolean` VB6 (`True=-1`), BSTR, `Variant` e `ByRef` seguono il contratto M3 finché una specifica MX approvata lo estende esplicitamente.
- **Regole di valutazione:** effetti collaterali, eccezioni, ordine di valutazione e cleanup devono essere dichiarati e testati.
- **Progetto interoperabile:** non imporre nuove dipendenze permanenti da repository upstream. Eventuale codice open source adottato deve essere integrato nei sorgenti VB6R rispettando licenze/NOTICE e decisioni architetturali vigenti.
- **Feature gating:** usare test di parsing/compilazione sia con estensioni abilitate sia disabilitate; gli errori devono menzionare la feature richiesta e l'opzione corrispondente.

## 7. Sequenza di sviluppo futura, non autorizzata ora

| Fase futura | Candidati | Gate d'ingresso |
|---|---|---|
| MX-P0 — Low-risk language ergonomics | S01 Return, S02 Continue, S03 AndAlso/OrElse | Parser/AST/IR e control flow VB6R stabili |
| MX-P1 — Tipi e stringhe | S04 As Auto, S05 interpolation, S08 Option Strict | Binding, type checking e conversioni M3 verificati |
| MX-P2 — Sicurezza/errori/risorse | S06 Try/Catch/Finally, S07 Using, S09 ?., S10 ?? | Unwinding, Err, nullability, ownership e cleanup definiti |
| MX-P3 — Collezioni e chiamate | S11 spread, S12 literals, S13 destructuring | Array/ParamArray/ByRef e tuple/UDT adeguati |
| MX-P4 — Funzioni e tipi evoluti | S14 lambda/closure, S15 generics, S16 pattern matching | Call ABI, capture/lifetime, tipi procedura, semantic analyzer generalizzato |
| MX-Task — Runtime callback | `VBTask.Then` / `.Catch` | Nuovi frame VM, callback e lifetime definiti; nessun Await |

L'ordine è adattabile in base alle dipendenze tecniche, mantenendo tutte le estensioni nell'ambito progettuale concordato.

## 8. Accettazione futura delle funzionalità MX

Per ciascuna feature, prima di dichiararla `IMPLEMENTED`, richiedere:

1. Specifica grammaticale senza ambiguità con VB6 legacy.
2. Nodo AST e diagnostica, inclusi casi negativi.
3. Contratto semantico e type checking, con eventuali decisioni aperte risolte.
4. Lowering automatico a IR VB6R versionato, **senza IR scritto a mano per sostituire la pipeline**.
5. Esecuzione reale in VM/runtime, o errore compile-time esplicito quando la feature non è supportata.
6. Test con modalità estesa attiva e test di regressione con modalità legacy.
7. Test su valutazione una sola volta, ordine degli effetti, errori e cleanup quando applicabile.
8. Esempio end-to-end compilato, eseguito e documentato.
9. Nessuna falsa dichiarazione di supporto del P-Code Microsoft o di compatibilità differenziale.

La milestone MX non potrà essere dichiarata conclusa sulla base del solo parsing della nuova sintassi.

## 9. Decisioni fissate e questioni aperte

### Fissate dall'owner

- Tutte le sedici estensioni della sezione 2 sono approvate **come obiettivi futuri**.
- La compatibilità VB6 legacy è un vincolo primario.
- `VBTask.Then/Catch` impiegano nuove invocazioni VM; niente Async/Await o frame sospesi.
- Le estensioni MX non alterano gli opcode Microsoft.
- **Non iniziare l'implementazione MX sulla base di questo documento.**

### Da definire quando MX verrà avviato

- Sintassi definitiva e ambito di `Option VB6RExtensions On` (modulo/progetto/file) e interazione con `Option Explicit` / `Option Strict`.
- Risoluzione grammaticale di `Return`, array literal `[ ... ]` e identificatori VB6 delimitati.
- Semantica precisa di `?.`, `??`, `AndAlso`, `OrElse` in presenza di `Null`, `Nothing`, `Empty` e `Variant`.
- Protocollo e garanzie di `Using`/`Finally`, inclusi errori durante la pulizia.
- Trattamento di `Try/Catch` insieme a `On Error` e `Err`.
- Tipi per array literals, destructuring, lambda/closure, generics e pattern matching.
- Modello di callback `VBTask` con GUI e thread; reentrancy e lifetime.
- Priorità effettive, compatibilità dei target e versione dell'IR che introdurrà ogni feature.

## 10. Conservazione e riapertura della specifica

**Collocazione consigliata:** `docs/MX-SYNTAX.md` (oppure nella sezione `docs` già dedicata a MX).
**Azione attuale:** conservare il documento per progettazione futura, **senza cambiare codice, gate M5/M6, catalogo opcode o milestone in corso**.
**Azione alla riapertura:** ratificare le questioni aperte, definire le acceptance fixture e autorizzare esplicitamente l'implementazione per fasi.
