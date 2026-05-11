# Visionneuse d'Images Forensiques en Rust

Une application GUI native en Rust pour analyser les images JPEG avec trois modes d'analyse forensique.

## Fonctionnalités

### Modes de visualisation

1. **Vue Classique** - Affichage original de l'image
2. **Error Level Analysis (ELA)** - Détecte les zones recompressées en comparant l'image originale avec une version compressée à 90%
3. **JPEG Ghost** - Révèle les artefacts de recompression en comparant deux niveaux de qualité différents (95% vs 85%)

## Technologies

- **eframe/egui** - Interface graphique native cross-platform
- **image** - Traitement d'images
- **rfd** - Sélecteur de fichiers natif

## Installation

### Prérequis

- Rust 1.70+
- Cargo

### Build

```bash
cargo build --release
```

### Exécution

```bash
cargo run --release
```

## Utilisation

1. Lancez l'application
2. Cliquez sur "Load JPEG Image" pour sélectionner un fichier JPEG
3. Utilisez les boutons "Classic", "ELA", "JPEG Ghost" pour basculer entre les modes
4. L'image traitée s'affiche en temps réel dans la fenêtre

## Architecture

- **Traitement d'image** : Utilise la bibliothèque `image` pour charger et sauvegarder temporairement les images
- **Analyse ELA** : Compresse l'image à 90% et calcule la différence absolue avec l'original
- **Analyse JPEG Ghost** : Compresse à deux qualités différentes et compare les différences
- **Amplification** : Les différences sont amplifiées pour rendre les artefacts visibles

## Limitations

- Support uniquement JPEG (comme demandé)
- Les très grandes images peuvent utiliser beaucoup de RAM
- Pas de sauvegarde des résultats (affichage uniquement)

## Optimisations

Cette version utilise egui (plus léger que iced) pour une interface simple et performante.