# Installare Forma su Windows

*English: [INSTALL.md](INSTALL.md)*

Forma è un'app portatile: non c'è un programma di installazione, non scrive nel registro e
può stare in qualsiasi cartella.

## Requisiti

- Windows 10 o Windows 11 a 64 bit (x64)
- Un driver grafico con supporto DirectX 12 o Vulkan (qualsiasi PC o portatile non troppo vecchio)
- Circa 50 MB di spazio su disco (il download è di circa 14 MB)

## Passaggi

1. Apri la pagina dell'[ultima release](https://github.com/filippovisentin/forma/releases/latest).
2. Sotto **Assets** fai clic su **`Forma-windows.zip`** per scaricarlo.
   (Il file `forma.exe` separato è lo stesso programma senza i file di accompagnamento.)
3. In Esplora file, clic destro sullo zip scaricato → **Estrai tutto…** e scegli una cartella,
   per esempio `Documenti\Forma`. Non avviare Forma dall'interno dello zip.
4. Apri la cartella `Forma` estratta. Contiene:
   - `forma.exe` — l'applicazione
   - `forma-cli.exe` — la versione a riga di comando (info sui file, script, render PNG)
   - `LEGGIMI.txt` — una breve guida d'uso
   - `LICENSE-MIT`, `LICENSE-APACHE`, `ATTRIBUTION.md` — licenze
5. Fai doppio clic su **`forma.exe`**.
6. La prima volta Windows può mostrare un riquadro blu **"PC protetto da Windows"**.
   È Microsoft SmartScreen: Forma non è firmata digitalmente, quindi Windows non la conosce
   ancora. Fai clic su **Ulteriori informazioni**, controlla che l'app sia `forma.exe`, poi su
   **Esegui comunque**. Windows ricorda la scelta per quel file.

Facoltativo: clic destro su `forma.exe` → **Mostra altre opzioni → Invia a → Desktop (crea
collegamento)**, oppure aggiungila a Start o alla barra delle applicazioni.

## Aprire i file

- **File → Open** o **Ctrl+O**, oppure trascina un file `.3dm` sulla finestra di Forma.
- Per aprire i `.3dm` con Forma con un doppio clic: clic destro su un file `.3dm` →
  **Apri con → Scegli un'altra app → Altre app → Cerca un'altra app nel PC** e seleziona
  `forma.exe`. Lascia deselezionato "Usa sempre questa app" se Rhino deve restare l'app predefinita.

## Aggiornare

Scarica il nuovo `Forma-windows.zip` ed estrailo sopra la vecchia cartella (o in una nuova).
Le impostazioni sono salvate altrove e restano.

## Dove sono le impostazioni

Forma ricorda osnap attivi, Grid Snap, Ortho, Planar, SmartTrack, Gumball, griglia, filtro di
selezione, modalità di visualizzazione di ogni vista, scheda della barra strumenti, larghezza
dei pannelli, ultima cartella usata e gli ultimi 10 file. Tutto in un solo file di testo:

```
%APPDATA%\Forma\settings.txt
```

(Incolla `%APPDATA%\Forma` nella barra degli indirizzi di Esplora file per aprire la cartella.)
Cancella il file per tornare ai valori iniziali.

Forma non crea altri file oltre ai `.3dm` che salvi tu.

## Disinstallare

1. Cancella la cartella in cui hai estratto Forma.
2. Se vuoi, cancella anche `%APPDATA%\Forma` per eliminare le impostazioni.

Fine: non c'è altro installato.

## Problemi comuni

| Problema | Cosa fare |
|---|---|
| "PC protetto da Windows" senza il pulsante **Esegui comunque** | Fai prima clic su **Ulteriori informazioni**. Su un PC gestito (scuola, lavoro) l'amministratore può aver bloccato le app non firmate. |
| "VCRUNTIME140.dll non trovato" | Installa *Microsoft Visual C++ Redistributable (x64)* dal sito Microsoft e riavvia Forma. |
| La finestra si apre ma le viste restano vuote, o l'app si chiude subito | Aggiorna il driver della scheda grafica dal sito del produttore (Intel, AMD, NVIDIA). Forma richiede DirectX 12 o Vulkan. |
| Un `.3dm` si apre ma mancano oggetti | Blocchi, testi e quote non sono ancora visualizzati. Da un terminale, `forma-cli.exe info file.3dm` mostra cosa contiene il file. |
| Altro | Apri una [issue](https://github.com/filippovisentin/forma/issues/new/choose) allegando, se puoi, il file `.3dm` e uno screenshot. |
