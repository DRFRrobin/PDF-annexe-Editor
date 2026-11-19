# Changelog

Règle : à chaque mise à jour, le numéro de version augmente de 0.0.1 (modifier `APP_VERSION` dans `app.js` et ajouter une entrée ici).

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
