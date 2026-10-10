# Forma in 10 minuti: modellare una stanza

*English: [QUICKSTART.md](QUICKSTART.md)*

In questo tutorial modelli una stanza di 6 × 4 m con muri, pavimento, un tavolo e un vaso, e la
salvi in un file `.3dm` da aprire in Rhino. Tutto è in **centimetri**, l'unità predefinita dei
nuovi documenti di Forma. Serve Forma installata ([INSTALL.it.md](INSTALL.it.md)).

![La stanza finita](images/overview.jpg)

## 0. Cinque cose da sapere

1. **Scrivi e basta.** Quello che digiti va nella riga di comando in alto. Scrivi il nome del
   comando e premi **Invio** (o **Spazio**). L'autocompletamento aiuta: `rec` → `Rectangle`.
2. **Le coordinate** si scrivono `x,y` o `x,y,z`; `@dx,dy` è relativo all'ultimo punto.
   Separale con spazi o Invio: `Rectangle 0,0 600,400` e poi Invio.
3. **Esc** annulla qualsiasi cosa in corso. **Ctrl+Z** annulla l'ultima operazione.
4. **Trascinare col destro** ruota la vista Perspective e sposta le altre; la **rotella** fa lo
   zoom. **Clic destro** = Invio, e con la riga vuota ripete l'ultimo comando.
5. **Doppio clic sul titolo di una vista** (`Top ▾`, `Perspective ▾`) la massimizza; di nuovo
   per tornare alle quattro viste.

Nella barra di stato in basso deve comparire **Centimeters**. Se non c'è, scrivi `New` e Invio.

## 1. Layer per muri e pavimento (1 min)

Un layer si crea e diventa corrente con un solo comando:

```text
Layer Muri
LayerColor 200,80,60
```

`LayerColor` senza nome di layer colora il layer corrente. Il colore si cambia anche dal pannello
**Layers** a destra (clic sul quadratino colorato).

## 2. Il perimetro della stanza (1 min)

Fai clic nella vista **Top** e scrivi:

```text
Rectangle 0,0 600,400
```

Si può anche disegnare col mouse: scegli lo strumento rettangolo a sinistra, clicca il primo
angolo, muovi il mouse e guarda la misura **larghezza × altezza** accanto al cursore; scrivi `600`
e Invio per fissare la lunghezza.

## 3. Muri spessi 15 cm (2 min)

Fai un offset del perimetro verso l'interno, trasforma i due contorni in un anello piano ed
estrudilo:

1. Clicca il rettangolo per selezionarlo, scrivi `Offset` e Invio.
2. Il prompt dice `Side to offset ( Distance=10 )`. Clicca **Distance=10** (o scrivi
   `Distance=15`), inserisci `15`, poi clicca **dentro** il rettangolo. Compare un secondo contorno.
3. Seleziona i due contorni (trascina una finestra intorno, o scrivi `SelAll`) e scrivi
   `PlanarSrf`. Il contorno interno diventa un foro: ottieni un anello piano.
4. Scrivi `SelNone`, poi `SelLast` per selezionare l'anello, poi `ExtrudeSrf 280`.

Ora hai muri alti 280 cm. Con i muri selezionati scrivi `Volume`: 8.148.000 cm³, cioè
(600 × 400 − 570 × 370) × 280. I contorni e l'anello piano restano sotto: puoi lasciarli oppure
selezionarli e premere **Canc**.

> Per ora i solidi di Forma sono mesh (i solidi esatti arriveranno con il kernel OpenCascade).
> Si vedono, si agganciano con gli osnap, si misurano e si salvano correttamente, ma non ci sono
> ancora le booleane: porte e finestre si ottengono modellando i pezzi di muro attorno alle
> aperture (vedi [examples/soggiorno.txt](examples/soggiorno.txt)).

## 4. Push / pull: la stanza alta 3 m (1 min)

1. Nella vista Perspective tieni premuti **Ctrl+Maiusc** e clicca la **faccia superiore** dei muri.
   Si colora di giallo e compare una freccia arancione.
2. Trascina la freccia verso l'alto: l'etichetta mostra `Pull … cm`. Rilascia per applicare.
   Oppure **clicca** la freccia, scrivi `20` e Invio per un valore esatto (negativo per spingere
   dentro).
3. I muri ora sono alti 300 cm e le facce attorno si sono allungate.

![Push / pull con la misura in tempo reale](images/pushpull.jpg)

Funziona su qualsiasi faccia piana di un box o di un'estrusione. Da riga di comando:
`MoveFace #id <punto sulla faccia> <distanza>`, oppure `PushPull <direzione> <distanza>` sui
solidi selezionati.

## 5. Pavimento e tavolo (1 min)

```text
Layer Pavimento
LayerColor 196,178,150
Box 0,0 600,400 -10
Layer Arredi
LayerColor 150,105,70
Box 200,150 380,250 75
```

`Box` vuole due angoli della base e un'altezza; un'altezza negativa va verso il basso, così la
soletta sta sotto i muri. Il secondo box è il blocco di un tavolo 180 × 100, alto 75 cm.

## 6. Il gumball (2 min)

Clicca il tavolo. Compare il **gumball**: frecce, archi e quadratini.

- **Trascina una freccia** per spostare lungo quell'asse. Gli osnap guidano il trascinamento.
- **Clicca una freccia**, scrivi `50` e Invio per spostare esattamente di 50 cm.
- **Trascina un arco** (o cliccalo e scrivi `90`) per ruotare.
- **Trascina un quadratino** per spostare in un piano.
- Il **pallino su ogni freccia** estrude: su un solido tira o spinge la faccia da quella parte.
  Clicca il pallino blu del tavolo, scrivi `-3` e Invio: ora il tavolo è alto 72 cm.

Ora un vaso: scrivi `Circle 520,320 30`, seleziona il cerchio e trascina il **pallino blu** verso
l'alto (circa 60 cm). Una curva chiusa si estrude in un solido.

![Gumball e pannello Properties](images/gumball.jpg)

## 7. Colori e layer (1 min)

- Con il vaso selezionato apri il pannello **Properties** (F3): mostra layer, colore e dimensioni.
  Clicca un colore (oppure scegli un colore personalizzato e premi **Apply**), o scrivi
  `SetObjectColor 90,140,100`. `SetObjectColor ByLayer` torna al colore del layer.
- Per spostare oggetti su un altro layer: selezionali e scrivi `ChangeLayer Arredi`, oppure rendi
  corrente il layer nel pannello **Layers** e clicca **Move Selection Here**.
- Nel pannello **Layers** la lampadina nasconde / mostra un layer e il lucchetto lo blocca.
- Prova le modalità di visualizzazione dal menu del titolo della vista (`Perspective ▾`):
  Wireframe, Shaded, Ghosted, X-Ray.

## 8. Salvare e aprire in Rhino (1 min)

1. **Ctrl+S** (o **File → Save**), scegli una cartella e un nome, per esempio `stanza.3dm`.
   Finché ci sono modifiche non salvate il titolo della finestra mostra `*`.
2. Apri `stanza.3dm` in Rhino (qualsiasi versione recente lo legge). Troverai:
   - i layer `Muri`, `Pavimento`, `Arredi` con i loro colori, e le unità in centimetri;
   - i contorni e il cerchio del vaso come curve esatte;
   - muri, pavimento, tavolo e vaso come **mesh**.
3. Al contrario, Forma apre i file di Rhino per vederli e fare piccole modifiche, ma solidi e
   superfici di Rhino diventano mesh e le curve diventano polilinee. Quando salvi un file così,
   Forma propone un nuovo nome (`nome-forma.3dm`) per non sovrascrivere mai l'originale.

Se in Rhino qualcosa non torna,
[apri una issue](https://github.com/filippovisentin/forma/issues/new/choose) allegando il file.

## E poi

- `Help` (o F1) elenca tutti i comandi con la loro sintassi.
- Strumenti curve nella scheda **Curve Tools**: `Offset`, `Trim`, `Split`, `Extend`, `Fillet`,
  `FilletCorners`, `Chamfer`, `Join`.
- `Array`, `ArrayLinear`, `ArrayPolar` per le sedie attorno al tavolo.
- La stanza degli screenshot è uno script: esegui `forma-cli run --file soggiorno.txt` con
  [examples/soggiorno.txt](examples/soggiorno.txt) e apri il `soggiorno.3dm` che ne risulta.
