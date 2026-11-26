<p align="center"><img src="../assets/logo-256.png" width="96" alt="Logo"></p>

# Annexio — version Rust

**Annexio** — application de bureau (Windows, Linux, macOS) : ajoute dans un rectangle rouge, à l'endroit choisi, le texte « Annexe X Folio n/N » sur les pages d'un PDF. Vos PDF ne quittent jamais l'ordinateur (seule la vérification de mise à jour, désactivable, contacte GitHub).

## Téléchargement

`annexio-windows-x86_64.exe` (ou Linux / macOS) (fichier fourni avec la release). Double-clic pour lancer.

## Utilisation

Barre d'onglets en haut, barre d'état en bas (bouton **Générer le PDF…** toujours visible), miniatures des pages à gauche de l'aperçu.

- **📄 Fichier** : ouvrir un PDF (Ctrl+O, bouton ou glisser-déposer), fichiers récents, navigation, fermer.
- **⚙ Paramètres** :
  - *Tampon* : texte (`{annexe}`, `{n}`, `{total}`) et modèles, premier folio, total, taille, pages concernées (Toutes / Paires / Impaires / Page courante ou `1-3,5`), fond blanc, annuler / rétablir.
  - *Position* : coins, centre, marge.
  - *Apparence* : thème Système / Clair / Sombre, taille de l'interface (80–200 %), miniatures.
  - *Raccourcis clavier*.

Placez le rectangle rouge en le glissant, en cliquant sur la page, ou avec les flèches (1 pt, Maj : 10 pt). La position est la même (en proportion) sur toutes les pages.

Aperçu : molette = changer de page ; Ctrl+molette ou boutons − / + = zoom (la molette fait défiler la page quand elle est agrandie) ; Page préc./suiv., Début/Fin = naviguer. La largeur du panneau de gauche se règle en glissant sa bordure.

| Raccourci | Action |
|---|---|
| Ctrl+O / Ctrl+S / Ctrl+W | Ouvrir / Générer / Fermer |
| Ctrl+Z / Ctrl+Y | Annuler / Rétablir |
| Ctrl + / Ctrl − | Taille de l'interface |
| Échap | Quitter un champ de saisie |

Les réglages (texte, position, thème, taille, fichiers récents) sont mémorisés dans un fichier local (`%APPDATA%\Annexio\settings.txt` sous Windows). ## Mise à jour automatique

Au démarrage, Annexio vérifie en arrière-plan s'il existe une version plus récente (requête vers `api.github.com` ; aucun fichier n'est envoyé). Un bandeau propose **Mettre à jour** ou **Plus tard**. L'installation télécharge le fichier de la release pour votre système, vérifie sa taille et son empreinte SHA-256, remplace l'exécutable (l'ancien est conservé en `.old` jusqu'au lancement suivant) puis propose de redémarrer.

Désactivable dans **Paramètres → Application** (« Vérifier les mises à jour au démarrage »), où un bouton permet aussi de rechercher à la demande. Le dossier de l'exécutable doit être modifiable par l'utilisateur (évitez `C:\Program Files`). La version de `Cargo.toml` doit correspondre au tag de la release (ex. `1.1.17` ↔ `v1.1.17`).

Un PDF peut aussi être passé en argument ou « ouvert avec » l'application.

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
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/annexio.wasm
```

`stamp_pdf(bytes: Uint8Array, optionsJson: string) -> Uint8Array`
(options : `annexe`, `template`, `start`, `total`, `fontSize`, `pages`, `whiteFill`, `fx`, `fy`).
