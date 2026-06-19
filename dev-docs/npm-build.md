# Construire les paquets npm (Node natif + WASM)

Ce guide décrit comment produire les **deux** paquets JS/TS à partir du même cœur Rust
(`src/core.rs`). L'analyse de conception est dans [javascript-typescript.md](javascript-typescript.md) ;
ce document ne couvre que les **commandes de build**.

Les deux bindings sont gardés par des *feature flags* Cargo, indépendants de PyO3 :

| Cible | Feature | Module Rust | Outil | Sortie |
|---|---|---|---|---|
| Node.js (natif) | `node` | `src/node.rs` | `@napi-rs/cli` | `dist/` (`*.node` + `index.js` + `index.d.ts`) |
| Navigateur (WASM) | `wasm` | `src/wasm.rs` | `wasm-pack` | `dist/wasm/` (`*.wasm` + `*.js` + `*.d.ts`) |

> Tous les fichiers générés (`dist/`, `pkg/`, `node_modules/`) sont dans `.gitignore` :
> ils se reconstruisent, on ne les versionne pas.

---

## 1. Node.js natif (napi-rs)

### Prérequis
- Node.js + npm
- Toolchain Rust (cible hôte)

### Build local
```bash
npm install                 # installe @napi-rs/cli (devDependency)
npm run build:debug         # napi build --platform --features node  -> debug
npm run build               # napi build --platform --release --features node
```

Produit dans `dist/` (via `napi build dist ... --dts index.d.ts`) :
- `protoruf.<triple>.node` — l'addon natif (ex. `protoruf.linux-x64-gnu.node`)
- `index.js` — le *loader* qui choisit le bon `.node`
- `index.d.ts` — les types TypeScript (générés depuis les annotations `#[napi]`)

> `package.json` pointe `main`/`types` vers `dist/index.js` / `dist/index.d.ts`.

### Vérifier
```js
import { compileProtoFromSources, jsonToProtobuf, protobufToJson, DescriptorCache } from "./dist/index.js";
const desc = compileProtoFromSources(
  { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; }' },
  "user.proto",
);
const pb = jsonToProtobuf('{"id":"123"}', desc, "user.User");
console.log(protobufToJson(pb, desc, false, "user.User")); // {"id":"123"}
```

> `compile_proto` (lecture disque) **est** exposé côté Node ; le navigateur (WASM) ne l'a pas.

---

## 2. Navigateur (wasm-pack)

### Prérequis
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack        # ou : curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Build
Les cibles `nodejs` et `web` produisent un `protoruf.js` **différent** ; on les sort donc
dans des dossiers séparés pour ne pas qu'elles s'écrasent :
```bash
wasm-pack build --target nodejs --out-dir dist/wasm     -- --features wasm  # exécutable en Node
wasm-pack build --target web    --out-dir dist/wasm-web -- --features wasm  # navigateur (init async)
# autre cible : --target bundler (Vite/webpack)
```

Produit `protoruf_bg.wasm`, `protoruf.js`, `protoruf.d.ts`, `package.json` dans le dossier choisi.

> **Nom de paquet scoping** : wasm-pack reprend le nom du crate (`protoruf`). Pour publier sous
> `@protoruf/wasm`, ajouter `--scope protoruf` (donne `@protoruf/protoruf`) puis ajuster
> le `package.json` généré, ou éditer le `name` après build.

### Note d'usage navigateur
Voir l'exemple complet [`examples/js/browser-example.html`](../examples/js/browser-example.html).
Le module s'initialise de façon **asynchrone** ; il faut le servir en HTTP (pas `file://`) :
```ts
import init, { compileProtoFromSources } from "../../dist/wasm-web/protoruf.js";
await init();                                   // charge et instancie le .wasm
const desc = compileProtoFromSources({ "user.proto": "..." }, "user.proto");
```
`compileProtoFromSources` accepte un objet `{ nomFichier: source }` (typé `any` dans le `.d.ts`
généré, car il transite en `JsValue` ; on peut le resserrer avec un type TS applicatif).

---

## 3. Tests (Vitest)

Un corpus partagé (`tests/js/conversion.shared.ts`) est rejoué contre **les deux** bindings
(parité avec `src/core.rs` : round-trip, maps, enums, oneof, valeurs par défaut, int64, JSON
invalide, type inconnu, compilation en mémoire, `DescriptorCache`), plus un test de **parité
octet-pour-octet** node ↔ wasm.

```bash
npm run test:js        # rebuild napi + wasm (pretest:js) puis lance vitest
# ou, si dist/ est déjà à jour :
npx vitest run
```

| Fichier | Cible |
|---|---|
| `tests/js/node.test.ts` | binding napi (`dist/index.js`) |
| `tests/js/wasm.test.ts` | binding wasm cible nodejs (`dist/wasm/protoruf.js`) |
| `tests/js/parity.test.ts` | node vs wasm : bytes identiques |

## 4. CI/CD (non implémenté)

Le workflow `publish-npm.yml` (matrice native + job wasm unique + job `publish`) reste à écrire ;
voir §7 de [javascript-typescript.md](javascript-typescript.md). Idem pour la couche TS idiomatique
(Zod : `objectToProtobuf` / `protobufToObject`).
