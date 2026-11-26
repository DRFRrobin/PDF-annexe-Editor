# Changelog

Règle : à chaque mise à jour, le numéro de version augmente de 0.0.1 (modifier `APP_VERSION` dans `app.js` et ajouter une entrée ici).

## 1.1.21
- **Application Rust : même parcours que la version web** — barre à chevrons Fichier → ReeMaker → Annexio → Export (Alt+1…4), ⚙ Paramètres en fenêtre (apparence, tampons, modèle de nom, mises à jour, raccourcis).
- Étape **ReeMaker** : cartouche à 3 lignes dessiné en vectoriel dans le PDF (rotation de page gérée), foliotage page par page (clic = placer, Entrée / clic droit = valider, Passer, Précédente, Pivoter, Tampon −/+, garder la position, Annuler), folios continus sans trou, zoom, orientation 0/90/180/270° ou **automatique** (sens de lecture du texte lu dans le contenu de la page).
- Étape **Annexio** : le tampon « Annexe X Folio n/N » existant (désormais placé en bas à gauche par défaut). Étape **Export** : récapitulatif et **nom automatique** `REE … Tr … Ind ….pdf` (modèle réglable) ; un seul PDF avec les deux tampons.
- Miniatures (pastille = page tamponnée), zoom, annuler/rétablir sur l'étape Annexio, réglages ReeMaker mémorisés.
- Version web : inchangée (seul le numéro suit).

## 1.1.20
- **Nom du PDF exporté automatique** d'après ReeMaker : « REE {ree} Tr {tranche} Ind {indice} » (ex. `REE JPJ 701 Tr 0 Ind A.pdf`, `Tr 1,2` si plusieurs tranches). Les parties vides disparaissent ; si ReeMaker est ignoré ou sans REE/indice, le nom reste « <original>_folioté.pdf ». Le nom se met à jour tout seul tant qu'on ne l'a pas modifié à la main (bouton « ↺ Nom automatique »).
- Modèle de nom réglable dans ⚙ (variables {ree} {tranche} {indice} {site} {arret} {cycle} {nom} {folio1} {folio2} ; `[ … ]` = partie facultative).
- À l'export, le navigateur ouvre la vraie boîte « Enregistrer sous » avec ce nom (Chrome, Edge) ; sinon téléchargement classique.

## 1.1.19
- Cartouche ReeMaker : le numéro de tranche est précédé de « Tr » (« Site de … Tr 0 », « Tr 1, 2 », « Tr 1-4 »).

## 1.1.18
- **Version web** : nouvelle interface en 4 étapes à chevrons (Fichier → ReeMaker → Annexio → Export), façon logiciel de montage (proposition A). Aperçu commun avec miniatures, zoom (Ctrl+molette) et calques visibles ; thème Système / Clair / Sombre ; réglages mémorisés.
- Nouvelle étape **ReeMaker** : cartouche à 3 lignes (site + tranche, REE + indice, folio + type d'arrêt), foliotage page par page (clic = placer, Entrée / clic droit = valider, Passer, Précédente, garder la position), folios continus, zoom, orientation 0/90/180/270° ou **automatique** (sens de lecture du texte de la page), pages pivotées gérées.
- L'étape **Annexio** reprend le tampon « Annexe X Folio n/N » existant (modèle, folio, total, taille, pages, positions rapides). L'export cumule les deux tampons dans un seul PDF.
- Code de dessin isolé dans `stamps.js`, avec tests (`node tests/stamps.test.js`).
- Hypothèses par défaut (à confirmer) : plusieurs tranches cochées → « 1, 2 » (ou « 1-4 » si consécutives) ; le cycle n'apparaît dans le cartouche que si la case est cochée ; clic droit ou Entrée valident ; un folio par page foliotée (les pages passées ne consomment pas de numéro). Non repris pour l'instant : tampons personnalisés. La version Rust n'est pas encore modifiée.

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
