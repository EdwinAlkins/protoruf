# Migration napi-rs 2 → 3 (binding Node.js)

Plan de migration du binding Node.js de **napi-rs 2** vers **napi-rs 3**.

> **TL;DR** — Migration **maintenance + cohérence d'API**, **pas** performance.
> Ne change rien à la vitesse de conversion ; unifie l'API sur `Uint8Array` et
> remet le binding sur la branche maintenue de napi-rs. À faire dans une PR
> dédiée, séparée de tout bump de dépendances perf.

## Pourquoi (et pourquoi pas)

### Performance : aucun gain attendu (~0 %)

napi n'est qu'une **fine couche FFI** autour de `core::*`. Tout le travail coûteux
(prost-reflect decode/encode, serde_json) vit dans le cœur Rust et est **identique
quelle que soit la version de napi**. Sur ce binding précis :

- descriptor `Buffer` → `&[u8]` : déjà un emprunt, **zéro copie** ;
- sortie `Buffer::from(Vec<u8>)` : napi **prend possession** du `Vec`, **zéro copie** ;
- seule copie réelle : la `String` JSON d'entrée (UTF-8), que napi 3 ne supprime pas.

Le seul levier touché par napi 3 serait le coût de marshalling à la frontière
(~ sous-µs), négligeable devant la conversion. Le gain de perf obtenu via
`prost`/`lru` ne passe d'ailleurs **même pas** par napi (les benchmarks tapent
`core::*` directement). **Ne pas migrer pour la vitesse.**

### Code / maintenance : bénéfice modeste mais réel

1. **Cohérence d'API `Uint8Array`** — le vrai bénéfice. Aujourd'hui `src/node.rs`
   expose `Buffer`, `src/wasm.rs` expose `Uint8Array`. Le `.d.ts` généré diffère
   donc entre les deux bindings, alors que `tests/js/conversion.shared.ts` type
   déjà tout en `Uint8Array` (ça marche au runtime car `Buffer extends
   Uint8Array`). napi 3 pousse vers `Uint8Array` → on **unifie** l'API node/wasm.
2. **Ligne maintenue** — napi-rs 2 est en maintenance ; v3 est la branche active
   (support Node/Rust récents, correctifs sécurité). Rester en v2 = dette.
3. **Génération TS / ESM** — `.d.ts` plus propre, sortie ESM possible.
4. **Churn Rust quasi nul** — le code `#[napi]` ne change presque pas.

## Contrainte clé : migration couplée JS + Rust

C'est **la** raison pour laquelle un simple bump Cargo échoue : avec le crate Rust
en v3 mais `@napi-rs/cli` en v2, la macro `#[napi]` **panique** à la génération
(« custom attribute panicked »). Il faut bumper **ensemble** :

- les crates Rust `napi` / `napi-derive` → `3` ;
- l'outil JS `@napi-rs/cli` → `^3`.

`napi-build` reste en `2.x` (pas de v3 publiée, compatible) ; `build.rs` est inchangé.

## Surface de migration

| Zone | Fichier | Changement |
|------|---------|------------|
| Tooling JS | `package.json` (`devDependencies`) | `@napi-rs/cli` `^2.18.4` → `^3` |
| Config napi | `package.json` (`napi`) | schéma v3 : `napi.triples { defaults, additional }` → `napi.targets` (tableau plat de triples) |
| Scripts build | `package.json` (`scripts`) | flags `napi build` v3 (`--platform`, `--dts` ont évolué) |
| Cargo | `Cargo.toml` | `napi = "3"`, `napi-derive = "3"` (testé : compile) |
| Rust | `src/node.rs` | imports `napi::bindgen_prelude` éventuels ; *(optionnel)* `Buffer` → `Uint8Array` |
| Build script | `build.rs` | inchangé (`napi-build` reste 2.x) |
| Glue générée | `dist/index.js`, `dist/index.d.ts` | régénérés par le CLI v3 (loader différent) — **gitignoré**, rien de suivi |
| Packaging | `scripts/pack-github.mjs` | à re-tester : il bundle `index.js`/`index.d.ts`/`*.node` en supposant le loader v2 |
| CI | `.github/workflows/release.yml` (l.59-60), `npm-test.yml` | aligner les flags `napi build` v3 |

## Étapes

### 1. Tooling JS

```bash
npm install --save-dev @napi-rs/cli@^3
```

Puis adapter la config `napi` de `package.json`. En v3, `triples` devient `targets` :

```jsonc
// Avant (cli v2)
"napi": {
  "name": "protoruf",
  "triples": {
    "defaults": true,
    "additional": [
      "aarch64-apple-darwin",
      "aarch64-unknown-linux-gnu",
      "x86_64-unknown-linux-gnu",
      "x86_64-pc-windows-msvc"
    ]
  }
}

// Après (cli v3) — vérifier le schéma exact avec `napi --help` v3
"napi": {
  "binaryName": "protoruf",
  "targets": [
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc"
  ]
}
```

> ⚠️ Vérifier le schéma et les flags réels avec la v3 installée (`npx napi build --help`) :
> `--platform`, `--dts` et la cible de sortie ont changé entre v2 et v3. Ne pas
> recopier ce bloc à l'aveugle.

### 2. Cargo

```toml
napi = { version = "3", optional = true, features = ["napi8"] }
napi-derive = { version = "3", optional = true }
# napi-build reste en "2"
```

### 3. Rust (`src/node.rs`) — minimal

- Ajuster les imports `napi::bindgen_prelude` si la v3 a déplacé des symboles.
- **Optionnel mais recommandé** : remplacer `Buffer` par `Uint8Array` en entrée et
  sortie pour aligner sur `wasm.rs`. Le `.d.ts` node devient alors identique au
  wasm, et `tests/js/conversion.shared.ts` (déjà typé `Uint8Array`) reflète la
  réalité côté node aussi.

### 4. CI

`.github/workflows/release.yml` (≈ l.59-60) :

```bash
npx napi build dist --platform --release --features node --dts index.d.ts
```

→ aligner sur la syntaxe `napi build` v3. `npm-test.yml` passe par
`npm run test:js`, donc OK si les scripts `package.json` sont à jour.

### 5. Validation

```bash
# Build + tests JS (node napi-3 + wasm + parité)
npm run test:js          # 26 tests attendus

# Packaging GitHub multi-plateforme (point de vigilance loader v3)
npm run pack:github

# Build release
npm run build
```

## Risques

- **Faible côté runtime** : couche fine, les tests de parité couvrent node↔wasm.
- **Friction principale** : la config CLI v3 (`targets`, flags `napi build`) et la
  validation que `scripts/pack-github.mjs` survit au **nouveau format du loader**
  `dist/index.js` généré par la v3.
- **Effort** : ½ à 1 journée, **PR dédiée**.

## Rollback

Le binding v2 est entièrement fonctionnel. En cas de blocage :

```toml
napi = { version = "2", optional = true, features = ["napi8"] }
napi-derive = { version = "2", optional = true }
```

+ `@napi-rs/cli` `^2.18.4` dans `package.json` + restaurer la config `napi.triples`.
Aucune dépendance perf n'est liée à napi, donc le rollback n'affecte pas les autres bumps.

## Checklist

- [ ] `@napi-rs/cli@^3` installé, `napi`/`napi-derive` → `3` dans `Cargo.toml`
- [ ] Config `napi` de `package.json` migrée (`triples` → `targets`)
- [ ] Flags `napi build` v3 ajustés (scripts + CI)
- [ ] *(optionnel)* `Buffer` → `Uint8Array` dans `src/node.rs`
- [ ] `npm run test:js` vert (26 tests)
- [ ] `npm run pack:github` produit un bundle multi-plateforme valide
- [ ] `release.yml` et `npm-test.yml` à jour
- [ ] README/docs node inchangés au niveau API si on garde `Uint8Array` ↔ `Buffer` compatibles
