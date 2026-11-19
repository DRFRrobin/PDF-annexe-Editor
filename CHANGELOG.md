# Changelog

Règle : à chaque mise à jour, le numéro de version augmente de 0.0.1 (modifier `APP_VERSION` dans `app.js` et ajouter une entrée ici).

## 1.1.17
- Application Rust : mise à jour automatique. Au démarrage, Annexio vérifie en arrière-plan s'il existe une release plus récente ; un bandeau propose « Mettre à jour » (ou « Plus tard »). Le fichier est téléchargé, contrôlé (taille + empreinte SHA-256 fournie par GitHub), puis remplace l'exécutable (l'ancien devient `.old`, supprimé au lancement suivant). Bouton « Redémarrer maintenant ». Désactivable dans Paramètres → Application, avec bouton « Rechercher une mise à jour ». Disponible à partir de cette version : les versions précédentes doivent être remplacées à la main une fois.

## 1.1.16
- Le logiciel s'appelle désormais **Annexio** (web et Rust) : titre, fenêtre, exécutable `annexio-<système>`, bibliothèque, métadonnées de l'exe. Les réglages de l'ancien dossier `AnnexePDF` sont repris automatiquement. L'adresse du dépôt et du site web ne changent pas.

## 1.1.15
- Application Rust : thème Système / Clair / Sombre et taille de l'interface réglable (80–200 %, Ctrl + / Ctrl − aussi), mémorisés.

## 1.1.14
- Application Rust : zoom de l'aperçu (boutons, Ctrl+molette, « Page entière » / « Largeur »), colonne de miniatures (pastille rouge = page tamponnée), réglages et fichiers récents mémorisés, annuler / rétablir (Ctrl+Z / Ctrl+Y), messages d'erreur plus clairs, refus d'écraser le fichier d'origine.

## 1.1.13
- Application Rust : barre du bas toujours visible (« Générer le PDF… », état, boutons « Ouvrir le PDF » / « Ouvrir le dossier » après génération), raccourcis (Ctrl+O, Ctrl+S, Ctrl+W, Échap, Début/Fin), positions rapides (coins, centre, marge), flèches pour ajuster le tampon (Maj : ×10), info-bulles, modèles de texte, boutons Toutes / Paires / Impaires / Page courante. Les flèches ↑ ↓ déplacent désormais le tampon (pages : molette, Page préc./suiv.).

## 1.1.12
- Application Rust : suppression de tout lien avec GitHub (lien du dépôt et système de mise à jour intégré, qui interrogeait GitHub). L'application ne fait plus aucune requête réseau.

## 1.1.11
- Nouveau logo (icône de l'exe Windows, de la fenêtre Rust, favicon et en-tête de l'app web).

## 1.1.10
- Application Rust : la largeur du panneau de gauche se règle en glissant sa bordure (le curseur devient ↔).

## 1.1.9
- Application Rust : la molette de la souris (ou ↑ ↓ / Page préc. / Page suiv.) fait défiler les pages dans l'aperçu.

## 1.1.8
- Mise à jour intégrée à l'application : bouton « Rechercher une mise à jour » (manuel, aucun appel réseau au démarrage). Télécharge le fichier de la dernière release GitHub pour votre système, vérifie sa taille, remplace l'exécutable et propose de redémarrer.
- Barre d'onglets en haut de l'application : « Fichier » et « Paramètres ».
- Version Rust : plus de version en ligne de commande (application graphique uniquement) ; la release publie `pdf-annexe-<système>`.

## 1.1.7
- Interface graphique complète (Rust, egui) : ouverture/glisser-déposer d'un PDF, aperçu, rectangle rouge déplaçable, réglages, génération. Les releases contiennent l'application (`pdf-annexe-<système>`) et la ligne de commande (`pdf-annexe-cli-<système>`).

## 1.1.6
- Workflow de release : accepte les tags `v*` et `V*`, et peut être lancé à la main sur un tag existant.

## 1.1.5
- Workflow GitHub Actions : à chaque tag `vX.Y.Z`, compile `pdf-annexe` (Windows .exe, Linux, macOS) et publie une release.

## 1.1.4
- Ajout d'une version Rust (`rust/`) : bibliothèque, CLI et liaison WebAssembly.

## 1.1.3
- README : retrait du lien « Code source sur GitHub ».

## 1.1.2
- Bouton « Ouvrir le dépôt GitHub » bien visible dans le panneau de l'app.
- Correction : la page blanche n'apparaît plus avant le chargement d'un PDF.

## 1.1.1
- Ajout d'un lien vers le dépôt GitHub (app et README).

## 1.1.0
- Version initiale numérotée : tampon rouge « Annexe X Folio n/N », 100 % local.
