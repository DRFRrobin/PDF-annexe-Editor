# PDF Annexe Editor — version Rust

Application de bureau (Windows, Linux, macOS) : ajoute dans un rectangle rouge, à l'endroit choisi, le texte « Annexe X Folio n/N » sur les pages d'un PDF. 100 % local : votre fichier ne quitte jamais l'ordinateur.

## Téléchargement

`pdf-annexe-windows-x86_64.exe` (ou Linux / macOS) dans les [Releases](https://github.com/DRFRrobin/PDF-annexe-Editor/releases). Double-clic pour lancer.

## Utilisation

Barre d'onglets en haut :
- **📄 Fichier** : ouvrir un PDF (bouton ou glisser-déposer), naviguer entre les pages, générer le PDF tamponné, fermer le fichier.
- **⚙ Paramètres** : texte du tampon (`{annexe}`, `{n}`, `{total}`), premier folio, total, taille, pages concernées (ex. `1-3,5`), fond blanc ; section **Application** (version, mise à jour, lien GitHub).

Placez le rectangle rouge en le glissant dans l'aperçu, ou en cliquant sur la page. La position est la même (en proportion) sur toutes les pages. Un PDF peut aussi être passé en argument ou « ouvert avec » l'application.

### Mise à jour

Bouton « Rechercher une mise à jour » (Paramètres → Application). Il interroge uniquement `api.github.com`, télécharge le fichier de la dernière release correspondant à votre système, remplace l'exécutable (l'ancien devient `.old`, supprimé au lancement suivant), puis propose de redémarrer. Aucune vérification automatique au démarrage : rien ne sort sans votre clic.

## Développement

```bash
cd rust
cargo run --release [fichier.pdf]
cargo test
```

La bibliothèque (`src/lib.rs`, tamponnage avec `lopdf`) expose aussi une liaison WebAssembly :

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm --lib
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/pdf_annexe.wasm
```

`stamp_pdf(bytes: Uint8Array, optionsJson: string) -> Uint8Array`
(options : `annexe`, `template`, `start`, `total`, `fontSize`, `pages`, `whiteFill`, `fx`, `fy`).
