# Analyse : compiler protoruf et l'interfacer avec JS/TS

Cette note analyse comment **réutiliser le cœur Rust de `protoruf` pour exposer la même
interface en JavaScript / TypeScript** que celle disponible aujourd'hui en Python.

L'objectif n'est pas de réécrire la logique de conversion, mais de **rajouter une seconde
couche de binding** au-dessus du code déjà existant, exactement comme PyO3 le fait pour
Python.

---

## Sommaire

1. [Point de départ : une architecture déjà favorable](#1-point-de-départ--une-architecture-déjà-favorable)
2. [Deux voies de compilation possibles](#2-deux-voies-de-compilation-possibles)
3. [Mise en œuvre concrète](#3-mise-en-œuvre-concrète)
4. [Le point dur : `compile_proto` et le système de fichiers](#4-le-point-dur--compile_proto-et-le-système-de-fichiers)
5. [Interface TypeScript cible](#5-interface-typescript-cible)
6. [Build & distribution npm](#6-build--distribution-npm)
7. [CI/CD : déployer Node natif et WASM](#7-cicd--déployer-node-natif-et-wasm)
8. [Tests avec Vitest (Node + WASM)](#8-tests-avec-vitest-node--wasm)
9. [Pièges à anticiper](#9-pièges-à-anticiper)
10. [Plan d'action proposé](#10-plan-daction-proposé)
11. [Annexe — correspondance Python ↔ JS/TS](#annexe--correspondance-python--jsts)
12. [Question ouverte — vecteur d'attaque cyber ?](#question-ouverte--la-compilation-protobuf-en-mémoire-dans-le-navigateur-est-elle-un-vecteur-dattaque-)

---

## 1. Point de départ : une architecture déjà favorable

Le projet est découpé proprement en deux couches :

| Fichier | Rôle | Dépendances |
|---|---|---|
| `src/core.rs` | Logique métier **pure Rust** : compilation `.proto`, JSON ↔ Protobuf, gestion du descriptor pool | `protox`, `prost`, `prost-reflect`, `serde_json` (aucune dépendance Python) |
| `src/lib.rs` | Fine couche de **binding PyO3** : conversion d'erreurs, types Python (`PyBytes`), classe `DescriptorCache` | `pyo3` |

C'est le point clé de toute cette analyse : **`core.rs` ne contient aucune dépendance à
Python.** Toutes les fonctions exposées en Python ne sont que des _wrappers_ d'une dizaine
de lignes autour de fonctions `core::*` qui renvoient des `Result<_, String>` :

```rust
// src/lib.rs — le binding ne fait que traduire types & erreurs
fn json_to_protobuf<'py>(py: Python<'py>, json_str: &str, descriptor_bytes: &[u8], message_type: &str)
    -> PyResult<Bound<'py, PyBytes>>
{
    let protobuf_bytes = core::json_to_protobuf_bytes(json_str, descriptor_bytes, message_type)
        .map_err(|e| PyErr::new::<PyValueError, _>(e))?;
    Ok(PyBytes::new(py, &protobuf_bytes))
}
```

> **Conséquence :** ajouter JS/TS revient à écrire un second `lib.rs` qui appelle exactement
> les mêmes fonctions `core::*`. Le cœur est partagé, jamais dupliqué.

### Surface d'API à reproduire

L'interface Python actuelle (`python/protoruf/`) :

| Symbole Python | Implémenté par | Notes |
|---|---|---|
| `compile_proto(proto_path, include_paths=None, output_path=None)` | Rust `compile_proto` + wrapper Python | lit le système de fichiers |
| `load_descriptor(path)` | Python pur | lecture de fichier |
| `json_to_protobuf(json_str, descriptor_bytes, message_type)` | Rust | |
| `protobuf_to_json(protobuf_bytes, descriptor_bytes, pretty=False, message_type=...)` | Rust | |
| `pydantic_to_protobuf(model, descriptor_bytes, message_type)` | Python pur (`model_dump_json` + Rust) | |
| `protobuf_to_pydantic(bytes, descriptor_bytes, model_class, message_type)` | Python pur (Rust + `model_validate_json`) | |
| `DescriptorCache(descriptor_bytes)` → `.json_to_protobuf(...)`, `.protobuf_to_json(...)` | Rust `#[pyclass]` | pool pré-décodé réutilisable |

En JS/TS, l'équivalent naturel de l'intégration Pydantic est **Zod** (ou un schéma TS
classique) : la couche « validation » reste du code TypeScript pur au-dessus de
`json_to_protobuf` / `protobuf_to_json`, comme `pydantic_*` est du Python pur.

---

## 2. Deux voies de compilation possibles

Pour viser JS/TS, deux technologies de binding Rust existent. Elles ne sont pas
exclusives, mais elles ne couvrent pas les mêmes cibles.

### Option A — WebAssembly (`wasm-bindgen` + `wasm-pack`)

Compile le crate en `.wasm` + glue JS générée, avec des types `.d.ts` automatiques.

- ✅ **Universel** : navigateur, Node.js, Deno, Bun, bundlers (Vite/webpack), edge runtimes.
- ✅ Génère **les types TypeScript automatiquement** depuis les signatures Rust.
- ✅ Distribution npm simple via `wasm-pack`.
- ⚠️ **Pas d'accès au système de fichiers** dans le navigateur → `compile_proto(path)`
  (qui appelle `protox::compile` sur des chemins disques) ne peut pas fonctionner tel quel
  côté navigateur (voir §4).
- ⚠️ Surcoût de (dé)sérialisation à la frontière JS↔WASM (les `&[u8]` passent par des copies
  mémoire), et binaire `.wasm` à charger.

### Option B — Addon natif Node (`napi-rs`)

Compile un `.node` (binaire natif) chargé directement par Node.js.

- ✅ **Performances natives**, pas de surcoût WASM.
- ✅ **Accès complet au système de fichiers** → `compile_proto` fonctionne à l'identique.
- ✅ Génère aussi les types `.d.ts`.
- ✅ Le modèle de distribution npm multi-plateforme de `napi-rs` est très proche de celui des
  _wheels_ `maturin` que le projet utilise déjà pour Python.
- ❌ **Node.js / Bun uniquement** : ne tourne pas dans le navigateur.
- ❌ Binaires précompilés par OS/arch (comme les wheels).

### Comparatif synthétique

| Critère | WASM (`wasm-pack`) | Natif (`napi-rs`) |
|---|---|---|
| Navigateur | ✅ | ❌ |
| Node.js / Bun / Deno | ✅ | ✅ (Node/Bun) |
| `compile_proto` depuis un fichier | ❌ (nécessite une variante « bytes », §4) | ✅ identique à Python |
| Perf brute | bonne, léger surcoût frontière | maximale |
| Génération `.d.ts` | ✅ | ✅ |
| Proximité avec le build actuel (maturin) | moyenne | forte |
| Distribution npm | simple | multi-binaires par plateforme |

### Recommandation

- **Cas d'usage backend / outillage / CI** (le besoin d'origine de protoruf) → **`napi-rs`**.
  C'est l'équivalent fidèle du binding Python : même accès disque, mêmes perfs, même modèle
  de packaging que les wheels.
- **Besoin navigateur** (conversion côté client) → **WASM**, en acceptant que `compile_proto`
  travaille sur des **bytes déjà chargés** plutôt que sur un chemin.

> Le plus robuste à terme : **réutiliser `core.rs` pour les deux**, et publier deux paquets
> (`@protoruf/node` en napi, `@protoruf/wasm` en wasm) partageant le même cœur. Si un seul
> paquet doit être choisi, **napi-rs** est le plus proche de l'existant Python.

---

## 3. Mise en œuvre concrète

### 3.1 Réorganisation du crate

Le cœur étant déjà isolé, il suffit d'ajouter un module de binding par cible, gardé par un
_feature flag_, pour ne pas mélanger les dépendances.

```
src/
├── core.rs        # inchangé — logique partagée
├── lib.rs         # binding Python (feature "python")
├── napi.rs        # NOUVEAU — binding Node (feature "node")
└── wasm.rs        # NOUVEAU — binding WASM (feature "wasm")
```

```toml
# Cargo.toml (extrait)
[features]
python = ["pyo3/extension-module"]
node   = ["napi", "napi-derive"]
wasm   = ["wasm-bindgen", "serde-wasm-bindgen"]

[dependencies]
# ... existant ...
napi          = { version = "2", optional = true, features = ["napi8"] }
napi-derive   = { version = "2", optional = true }
wasm-bindgen  = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }
```

> Le `crate-type` est déjà `["cdylib", "rlib"]`, ce qui convient aussi bien à WASM qu'à
> napi-rs — aucun changement nécessaire de ce côté.

### 3.2 Binding napi-rs (équivalent exact du binding Python)

```rust
// src/napi.rs
use napi_derive::napi;
use crate::core;

#[napi]
pub fn compile_proto(proto_path: String, include_paths: Option<Vec<String>>) -> napi::Result<Vec<u8>> {
    core::compile_proto(&proto_path, include_paths).map_err(|e| napi::Error::from_reason(e))
}

#[napi]
pub fn json_to_protobuf(json_str: String, descriptor_bytes: &[u8], message_type: String) -> napi::Result<Vec<u8>> {
    core::json_to_protobuf_bytes(&json_str, descriptor_bytes, &message_type)
        .map_err(|e| napi::Error::from_reason(e))
}

#[napi]
pub fn protobuf_to_json(protobuf_bytes: &[u8], descriptor_bytes: &[u8], pretty: Option<bool>, message_type: String) -> napi::Result<String> {
    core::protobuf_to_json_string(protobuf_bytes, descriptor_bytes, pretty.unwrap_or(false), &message_type)
        .map_err(|e| napi::Error::from_reason(e))
}

/// Équivalent de la classe `DescriptorCache` exposée en Python.
#[napi]
pub struct DescriptorCache {
    pool: prost_reflect::DescriptorPool,
    // (mémoïsation des descripteurs comme dans lib.rs)
}

#[napi]
impl DescriptorCache {
    #[napi(constructor)]
    pub fn new(descriptor_bytes: &[u8]) -> napi::Result<Self> {
        let pool = core::load_descriptor_pool(descriptor_bytes).map_err(napi::Error::from_reason)?;
        Ok(Self { pool })
    }

    #[napi]
    pub fn json_to_protobuf(&self, json_str: String, message_type: String) -> napi::Result<Vec<u8>> {
        let desc = core::get_message_descriptor(&self.pool, &message_type).map_err(napi::Error::from_reason)?;
        core::json_to_protobuf_bytes_with_descriptor(&json_str, &desc).map_err(napi::Error::from_reason)
    }

    #[napi]
    pub fn protobuf_to_json(&self, protobuf_bytes: &[u8], message_type: String, pretty: Option<bool>) -> napi::Result<String> {
        let desc = core::get_message_descriptor(&self.pool, &message_type).map_err(napi::Error::from_reason)?;
        core::protobuf_to_json_string_with_descriptor(protobuf_bytes, &desc, pretty.unwrap_or(false))
            .map_err(napi::Error::from_reason)
    }
}
```

On retrouve **les mêmes appels `core::*`** que dans `lib.rs` : seule la traduction
types/erreurs change (`PyErr` → `napi::Error`, `PyBytes` → `Vec<u8>`/`Buffer`).

### 3.3 Binding WASM

Identique dans l'esprit ; les `&[u8]` deviennent `Vec<u8>` (mappés sur `Uint8Array` côté JS),
et les erreurs `String` deviennent des `JsError`.

```rust
// src/wasm.rs
use wasm_bindgen::prelude::*;
use crate::core;

#[wasm_bindgen]
pub fn json_to_protobuf(json_str: &str, descriptor_bytes: &[u8], message_type: &str) -> Result<Vec<u8>, JsError> {
    core::json_to_protobuf_bytes(json_str, descriptor_bytes, message_type)
        .map_err(|e| JsError::new(&e))
}

#[wasm_bindgen]
pub fn protobuf_to_json(protobuf_bytes: &[u8], descriptor_bytes: &[u8], pretty: bool, message_type: &str) -> Result<String, JsError> {
    core::protobuf_to_json_string(protobuf_bytes, descriptor_bytes, pretty, message_type)
        .map_err(|e| JsError::new(&e))
}

// DescriptorCache : même structure, annotée #[wasm_bindgen].
```

---

## 4. Le point dur : `compile_proto` et le système de fichiers

C'est la **seule** vraie différence fonctionnelle entre les cibles.

`core::compile_proto` appelle `protox::compile(&[&proto_path], &include_paths_ref)`, qui
**lit les `.proto` (et leurs imports) sur le disque**. Or :

- **napi-rs (Node)** : le système de fichiers existe → `compile_proto(path)` marche **à
  l'identique** de Python. Rien à faire.
- **WASM navigateur** : pas de FS → impossible de compiler depuis un chemin.

> **Décision retenue.** L'interface **WASM n'utilisera pas `compile_proto(path)`** : elle devra
> compiler à partir d'une **chaîne de caractères** contenant la source du `.proto`. Cela impose
> une **petite évolution de `core.rs`** : on **n'modifie pas** la fonction existante, on **ajoute
> une fonction dédiée** `compile_proto_from_sources(contents, root)` qui traite ce cas à part.
> Ainsi `compile_proto` (basé chemin/FS) reste intact pour Python et Node, et la nouvelle
> fonction (basée sources en mémoire) sert WASM — tout en restant réutilisable par les trois
> cibles.

### Peut-on compiler un `.proto` (sous forme de chaîne) en mémoire dans le navigateur ?

**Oui, sans réserve.** C'est le cœur de la réponse à cette question. L'API de `protox` (vérifiée
sur la version `0.7.2` déjà utilisée par le projet) permet une compilation **100 % en mémoire**,
sans le moindre accès au système de fichiers. Trois faits le garantissent :

1. **`protox::file::File::from_source(name, source)`** parse un `.proto` directement depuis une
   chaîne. Son implémentation se réduit à `protox_parse::parse(name, source)` et renvoie un
   `File { path: None, source: Some(...), ... }` : **aucune I/O disque**.

2. Le trait **`FileResolver`** n'exige qu'une méthode, `open_file(name) -> Result<File>`. On
   peut donc en écrire une implémentation qui lit dans une `HashMap<String, String>` en
   mémoire. **Les `import`** entre protos sont résolus *par nom* via ce même resolver : toute
   l'arborescence (fichier racine + imports) peut donc vivre en RAM.

3. **`Compiler::with_file_resolver(resolver)`** suivi de `.open_file("root.proto")` puis
   `.encode_file_descriptor_set()` produit exactement le **même `descriptor_bytes`** que le
   `compile_proto` actuel.

De plus, les **types « well-known » de Google** (`google/protobuf/timestamp.proto`, `struct`,
`wrappers`, `any`, …) sont **embarqués dans le binaire** via `include_str!`
(`protox::file::GoogleFileResolver`). Un proto qui fait `import "google/protobuf/timestamp.proto"`
compile donc aussi en navigateur, sans disque. Enfin, `protox` est `#![deny(unsafe_code)]`, donc
parfaitement compatible WASM.

> Seule l'API actuelle **basée sur un chemin** ne marche pas en navigateur :
> `protox::compile(&[path], &includes)` (et donc `core::compile_proto`) s'appuie sur
> `IncludeFileResolver`, qui lit le FS. La solution n'est pas de la remplacer mais d'**ajouter une
> fonction sœur** prenant des sources en mémoire.

#### Fonction à ajouter dans `core.rs` (réutilisable par les 3 cibles)

```rust
// src/core.rs — compilation entièrement en mémoire, sans accès disque
use std::collections::HashMap;
use protox::Compiler;
use protox::file::{ChainFileResolver, GoogleFileResolver, FileResolver, File};
use protox::Error as ProtoxError;

/// Resolver qui sert des .proto stockés en mémoire (nom -> contenu source).
struct InMemoryResolver {
    files: HashMap<String, String>,
}

impl FileResolver for InMemoryResolver {
    fn open_file(&self, name: &str) -> Result<File, ProtoxError> {
        match self.files.get(name) {
            Some(source) => File::from_source(name, source),
            None => Err(ProtoxError::file_not_found(name)),
        }
    }
}

/// Compile un ensemble de .proto fournis en mémoire vers un descriptor set (bytes).
///
/// `files` mappe le nom logique de chaque fichier (ex: "user.proto",
/// "common/types.proto") à son contenu source. `root` désigne le fichier d'entrée.
/// Aucun accès au système de fichiers : fonctionne en WASM/navigateur.
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
) -> Result<Vec<u8>, String> {
    let mut resolver = ChainFileResolver::new();
    resolver.add(InMemoryResolver { files });   // priorité aux protos de l'utilisateur
    resolver.add(GoogleFileResolver::new());     // well-known types embarqués

    let mut compiler = Compiler::with_file_resolver(resolver);
    compiler
        .open_file(root)
        .map_err(|e| format!("Failed to compile proto: {}", e))?;
    Ok(compiler.encode_file_descriptor_set())
}
```

> `ChainFileResolver` essaie chaque resolver dans l'ordre : on met d'abord les fichiers de
> l'utilisateur, puis le `GoogleFileResolver` pour couvrir les imports `google/protobuf/*`.
> Pour un seul fichier sans import, un `HashMap` à une entrée suffit.

#### Exposition côté JS/TS

```rust
// WASM (src/wasm.rs) — on accepte une Map JS { nomFichier: contenu }
#[wasm_bindgen]
pub fn compile_proto_from_sources(files: JsValue, root: &str) -> Result<Vec<u8>, JsError> {
    let files: HashMap<String, String> = serde_wasm_bindgen::from_value(files)
        .map_err(|e| JsError::new(&e.to_string()))?;
    core::compile_proto_from_sources(files, root).map_err(|e| JsError::new(&e))
}
```

```typescript
// Utilisation navigateur — aucun accès disque
const descriptor = compileProtoFromSources(
  { "user.proto": "syntax = \"proto3\"; package user; message User { string id = 1; }" },
  "user.proto",
);
const bytes = jsonToProtobuf('{"id":"123"}', descriptor, "user.User");
```

#### Stratégies, par ordre de simplicité

1. **Compiler en amont, charger des bytes (le plus simple).** La compilation se fait une fois
   (CLI, build, ou côté Node) et le navigateur ne reçoit que `descriptor_bytes`. Reflète déjà le
   pattern `load_descriptor` de Python. À privilégier si les schémas sont figés.

2. **`compile_proto_from_sources(files, root)` (ci-dessus).** À utiliser dès que les `.proto`
   sont fournis dynamiquement côté client (éditeur en ligne, schéma saisi par l'utilisateur…).
   Un ajout net dans `core.rs`, partagé par Python, Node et WASM.

3. **FS virtuel WASI** (plus lourd, rarement justifié ici).

> Bilan : l'API « conversion » est portable à 100 %. La compilation l'est aussi **dès lors qu'on
> passe par les sources en mémoire** ; seule la variante « compilation depuis un chemin disque »
> reste réservée à Node/natif.

---

## 5. Interface TypeScript cible

Les deux outils génèrent automatiquement le `.d.ts`. La forme visée, miroir du `_protoruf.pyi` :

```typescript
// protoruf.d.ts (généré)

/**
 * Compile un fichier `.proto` en descriptor set (bytes).
 *
 * ⚠️ **Node/natif uniquement** : lit les `.proto` (et leurs imports) sur le disque.
 * En navigateur/WASM, utiliser {@link compileProtoFromSources} à la place.
 *
 * @param protoPath     Chemin du fichier `.proto` à compiler.
 * @param includePaths  Répertoires d'inclusion pour résoudre les `import`
 *                       (défaut : le dossier parent de `protoPath`).
 * @returns Le descriptor set sérialisé, à passer aux fonctions de conversion.
 * @throws Si la compilation échoue (syntaxe invalide, import introuvable…).
 *
 * @example
 * const descriptor = compileProto("schema.proto");
 */
export function compileProto(protoPath: string, includePaths?: string[]): Uint8Array;

/**
 * Compile des `.proto` fournis **en mémoire**, sans aucun accès disque.
 * Disponible partout, **y compris navigateur/WASM**.
 *
 * @param files  Map du nom logique de chaque fichier vers son contenu source
 *               (ex : `{ "user.proto": "syntax = \"proto3\"; ..." }`). Les `import`
 *               sont résolus par nom dans cette map ; les types « well-known » de
 *               Google (`google/protobuf/*.proto`) sont fournis automatiquement.
 * @param root   Nom du fichier d'entrée à compiler (doit être une clé de `files`).
 * @returns Le descriptor set sérialisé, identique à celui de {@link compileProto}.
 * @throws Si la compilation échoue ou si `root`/un import est absent de `files`.
 *
 * @example
 * const descriptor = compileProtoFromSources(
 *   { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; }' },
 *   "user.proto",
 * );
 */
export function compileProtoFromSources(files: Record<string, string>, root: string): Uint8Array;

/**
 * Convertit une chaîne JSON en message Protobuf (bytes).
 *
 * @param jsonStr          Chaîne JSON à convertir.
 * @param descriptorBytes  Descriptor set issu de {@link compileProto} /
 *                         {@link compileProtoFromSources}.
 * @param messageType      Nom pleinement qualifié du message (ex : `"user.User"`).
 * @returns Le message Protobuf encodé.
 * @throws Si le JSON est invalide ou si `messageType` est absent du descriptor.
 */
export function jsonToProtobuf(jsonStr: string, descriptorBytes: Uint8Array, messageType: string): Uint8Array;

/**
 * Convertit un message Protobuf (bytes) en chaîne JSON.
 *
 * @param protobufBytes    Message Protobuf encodé.
 * @param descriptorBytes  Descriptor set issu de {@link compileProto} /
 *                         {@link compileProtoFromSources}.
 * @param pretty           Si `true`, formate le JSON avec indentation (défaut : `false`).
 * @param messageType      Nom pleinement qualifié du message (ex : `"user.User"`).
 * @returns La représentation JSON du message.
 * @throws Si le décodage ou la sérialisation JSON échoue.
 */
export function protobufToJson(
  protobufBytes: Uint8Array,
  descriptorBytes: Uint8Array,
  pretty: boolean | undefined,
  messageType: string,
): string;

/**
 * Pool de descripteurs **pré-décodé et réutilisable**.
 *
 * Décoder le descriptor set est le coût dominant de chaque conversion. Construire
 * cette instance une fois et la réutiliser évite de re-décoder le pool (et de
 * re-résoudre les descripteurs de message) à chaque appel — levier de performance n°1.
 *
 * @example
 * const cache = new DescriptorCache(descriptor);
 * const bytes = cache.jsonToProtobuf('{"id":"123"}', "user.User");
 * const json  = cache.protobufToJson(bytes, "user.User");
 */
export class DescriptorCache {
  /**
   * @param descriptorBytes Descriptor set à décoder une fois et conserver.
   * @throws Si les bytes ne forment pas un descriptor set valide.
   */
  constructor(descriptorBytes: Uint8Array);

  /**
   * Convertit une chaîne JSON en message Protobuf, via le pool mis en cache.
   * @param jsonStr      Chaîne JSON à convertir.
   * @param messageType  Nom pleinement qualifié du message (ex : `"user.User"`).
   * @returns Le message Protobuf encodé.
   * @throws Si le JSON est invalide ou si `messageType` est absent du pool.
   */
  jsonToProtobuf(jsonStr: string, messageType: string): Uint8Array;

  /**
   * Convertit un message Protobuf en chaîne JSON, via le pool mis en cache.
   * @param protobufBytes  Message Protobuf encodé.
   * @param messageType    Nom pleinement qualifié du message (ex : `"user.User"`).
   * @param pretty         Si `true`, formate le JSON avec indentation (défaut : `false`).
   * @returns La représentation JSON du message.
   * @throws Si le décodage ou la sérialisation JSON échoue.
   */
  protobufToJson(protobufBytes: Uint8Array, messageType: string, pretty?: boolean): string;
}
```

Couche idiomatique TS au-dessus (équivalent des helpers `pydantic_*`), en pur TypeScript :

```typescript
// index.ts — sucre, comme compiler.py / pydantic_* le sont en Python
import { z } from "zod";

/**
 * Convertit un objet JS/TS en message Protobuf (équivalent de `pydantic_to_protobuf`).
 *
 * @param obj          Objet à sérialiser (sérialisé via `JSON.stringify`).
 * @param descriptor   Descriptor set issu de `compileProto*`.
 * @param messageType  Nom pleinement qualifié du message (ex : `"user.User"`).
 * @returns Le message Protobuf encodé.
 */
export function objectToProtobuf<T>(obj: T, descriptor: Uint8Array, messageType: string): Uint8Array {
  return jsonToProtobuf(JSON.stringify(obj), descriptor, messageType);
}

/**
 * Convertit un message Protobuf en objet typé et **validé par Zod**
 * (équivalent de `protobuf_to_pydantic`).
 *
 * @param bytes        Message Protobuf encodé.
 * @param descriptor   Descriptor set issu de `compileProto*`.
 * @param schema       Schéma Zod décrivant et validant la forme attendue.
 * @param messageType  Nom pleinement qualifié du message (ex : `"user.User"`).
 * @returns L'objet validé, typé `T` (le type inféré du schéma).
 * @throws {z.ZodError} Si la donnée décodée ne respecte pas le schéma.
 */
export function protobufToObject<T>(bytes: Uint8Array, descriptor: Uint8Array, schema: z.ZodType<T>, messageType: string): T {
  return schema.parse(JSON.parse(protobufToJson(bytes, descriptor, false, messageType)));
}
```

> Convention : exposer les noms en `camelCase` (`jsonToProtobuf`) pour rester idiomatique JS,
> tout en conservant la même sémantique d'arguments qu'en Python.

---

## 6. Build & distribution npm

### Avec napi-rs

```bash
npm install -g @napi-rs/cli
napi build --release --platform     # produit protoruf.node + index.d.ts
```

`napi-rs` génère un paquet principal + des paquets optionnels par plateforme
(`@protoruf/node-linux-x64-gnu`, `-darwin-arm64`, `-win32-x64-msvc`, …), sélectionnés à
l'installation via `optionalDependencies`. **C'est le pendant exact des wheels `maturin`**
publiées pour Python.

### Avec wasm-pack

```bash
cargo install wasm-pack
wasm-pack build --release --target web      # ou --target nodejs / bundler
```

Produit un dossier `pkg/` prêt à publier (`pkg/protoruf_bg.wasm`, `.js`, `.d.ts`,
`package.json`). Côté navigateur, la compilation des schémas se fait via
`compileProtoFromSources(...)` (chaîne de caractères) ; `compileProto(path)` n'y est pas exposé.

---

## 7. CI/CD : déployer Node natif et WASM

Le dépôt publie déjà sur PyPI via `.github/workflows/publish-pypi.yml` : une **matrice**
(`linux` / `windows` / `macos` / `sdist`) qui construit les artefacts avec
`PyO3/maturin-action`, les téléverse, puis un job `publish` final qui les agrège et pousse sur
PyPI (OIDC, `id-token: write`). **On calque exactement ce modèle pour npm.**

### Comment gérer les deux cibles (natif + WASM) ?

Ce sont **deux paquets npm distincts**, produits par **deux toolchains différentes**, donc deux
workflows (ou deux groupes de jobs) qui se rejoignent sur un job `publish` :

| | `@protoruf/node` (natif napi-rs) | `@protoruf/wasm` (wasm-pack) |
|---|---|---|
| Toolchain | `@napi-rs/cli` | `wasm-pack` |
| Build | **matriciel** (1 binaire `.node` par OS/arch) | **unique** (`.wasm` portable, `ubuntu-latest` suffit) |
| Analogie PyPI | les jobs `linux`/`windows`/`macos` (wheels) | proche du job `sdist` (artefact unique) |
| Cible `rustup` | hôte | `wasm32-unknown-unknown` |
| Publication | `npm publish` du paquet principal + paquets `optionalDependencies` par plateforme | `npm publish` du dossier `pkg/` |

Point clé du **natif** : comme les wheels, un addon `.node` est **spécifique à OS+arch**.
napi-rs gère ça nativement — le paquet principal déclare en `optionalDependencies` un sous-paquet
par plateforme (`@protoruf/node-linux-x64-gnu`, `-darwin-arm64`, `-win32-x64-msvc`, …), et npm
n'installe que celui correspondant à la machine du client. Le WASM, lui, est **un seul binaire
portable** : pas de matrice, un seul artefact.

### Workflow proposé (`.github/workflows/publish-npm.yml`)

```yaml
name: Publish to npm

on:
  release:
    types: [published]
  workflow_dispatch:

jobs:
  # 1) NATIF — une matrice OS/arch, miroir des jobs wheels
  build-native:
    strategy:
      fail-fast: false
      matrix:
        include:
          - { runner: ubuntu-latest,  target: x86_64-unknown-linux-gnu }
          - { runner: ubuntu-latest,  target: aarch64-unknown-linux-gnu }
          - { runner: macos-latest,   target: x86_64-apple-darwin }
          - { runner: macos-latest,   target: aarch64-apple-darwin }
          - { runner: windows-latest, target: x86_64-pc-windows-msvc }
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: actions/setup-node@v4
        with: { node-version: 20, registry-url: 'https://registry.npmjs.org' }
      - run: npm ci
      # napi build produit le .node + les sous-paquets par plateforme
      - run: npx @napi-rs/cli build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: napi-${{ matrix.target }}
          path: "*.node"

  # 2) WASM — un seul artefact portable (pas de matrice)
  build-wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
      # --target web (ou bundler) ; --features wasm pour le bon binding
      - run: wasm-pack build --release --target web --out-dir pkg -- --features wasm
      - uses: actions/upload-artifact@v4
        with: { name: wasm-pkg, path: pkg }

  # 3) PUBLISH — agrège tout et pousse les 2 paquets (miroir du job `publish` PyPI)
  publish:
    needs: [build-native, build-wasm]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20, registry-url: 'https://registry.npmjs.org' }
      - uses: actions/download-artifact@v4
        with: { path: artifacts }
      # napi assemble les binaires téléchargés dans les bons sous-paquets
      - run: npx @napi-rs/cli artifacts --dir artifacts
      - run: npm publish --access public          # @protoruf/node + sous-paquets plateforme
        working-directory: ./packages/node
      - run: npm publish --access public          # @protoruf/wasm
        working-directory: ./artifacts/wasm-pkg
    env:
      NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### Modifications / ajouts concrets à prévoir

- **Feature flags `Cargo.toml`** : ajouter `node` et `wasm` (cf. §3.1) pour que chaque job
  compile le bon binding sans embarquer PyO3.
- **Secret `NPM_TOKEN`** (token d'automatisation npm) dans les secrets du dépôt, ou
  **OIDC/Trusted Publishing npm** pour éviter un token longue durée — exactement l'esprit du
  `id-token: write` côté PyPI.
- **`package.json` napi-rs** : champs `napi.triples` listant les plateformes ciblées + les
  `optionalDependencies` générées par `napi-rs`. C'est le pendant de la matrice de wheels.
- **Cibles `rustup`** par job : `targets:` dans `dtolnay/rust-toolchain` (matrice native) et
  `wasm32-unknown-unknown` (job wasm).
- **Cross-compilation** Linux aarch64 : soit `cross`, soit l'image Docker fournie par
  `@napi-rs/cli`, à brancher dans le job correspondant.
- **Profil de build** : le `[profile.dist]` de `Cargo.toml` (LTO fat, `codegen-units = 1`,
  `strip`) s'applique aussi à ces builds ; on peut le réutiliser pour des binaires optimisés
  (`--profile dist`).
- **Déclenchement** : conserver le même que PyPI (`on: release: [published]` +
  `workflow_dispatch`) pour publier les trois écosystèmes (PyPI, npm natif, npm WASM) sur une
  même release Git.

> **Résumé de la stratégie « 2 cibles »** : **le natif suit la même logique matricielle que les
> wheels** (un binaire par OS/arch, agrégés en sous-paquets), **le WASM est un artefact unique
> portable**. Les deux convergent vers un job `publish` final, calqué sur celui déjà utilisé pour
> PyPI.

---

## 8. Tests avec Vitest (Node + WASM)

Comme la version Python est couverte par `pytest` et `core.rs` par `cargo test`, **les bindings
JS/TS doivent l'être par une suite de tests JavaScript**. La bibliothèque retenue est
**[Vitest](https://vitest.dev/)** : rapide, support TypeScript natif, et capable de tester **les
deux cibles** (Node natif et WASM) depuis une même base de tests.

### Pourquoi Vitest pour les deux cibles

- **TypeScript & ESM out-of-the-box** : aligné avec les types `.d.ts` générés (§5), sans config
  de transpilation.
- **Multi-environnements** : l'option `environment` (`node` vs `jsdom`/`happy-dom`) et les
  *workspaces* permettent de faire tourner **la même logique de test** contre le paquet
  `@protoruf/node` et contre le paquet `@protoruf/wasm`.
- **Support WASM** : Vitest (via Vite) sait charger un module `.wasm` ; l'init asynchrone du
  module WASM se gère dans un `beforeAll(async () => await init())`.

### Stratégie : un seul corpus de cas, deux cibles

L'idée est de **factoriser les assertions** (round-trip, maps, enums, oneof, valeurs par défaut,
JSON invalide, type de message inconnu — soit la **parité avec les cas de `core.rs`**) et de les
exécuter contre chaque binding via un *workspace* Vitest :

```ts
// vitest.config.ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    projects: [
      { test: { name: "node", environment: "node",     include: ["tests/**/*.node.test.ts"] } },
      { test: { name: "wasm", environment: "happy-dom", include: ["tests/**/*.wasm.test.ts"] } },
    ],
  },
});
```

```ts
// tests/conversion.shared.ts — assertions partagées, indépendantes de la cible
import { expect } from "vitest";

export interface ProtorufApi {
  compileProtoFromSources(files: Record<string, string>, root: string): Uint8Array;
  jsonToProtobuf(json: string, descriptor: Uint8Array, messageType: string): Uint8Array;
  protobufToJson(bytes: Uint8Array, descriptor: Uint8Array, pretty: boolean, messageType: string): string;
}

const PROTO = 'syntax="proto3"; package user; message User { string id = 1; repeated string tags = 2; }';

export function runConversionSuite(api: ProtorufApi) {
  const descriptor = api.compileProtoFromSources({ "user.proto": PROTO }, "user.proto");

  // Round-trip JSON -> Protobuf -> JSON
  const bytes = api.jsonToProtobuf('{"id":"123","tags":["a","b"]}', descriptor, "user.User");
  const json = JSON.parse(api.protobufToJson(bytes, descriptor, false, "user.User"));
  expect(json.id).toBe("123");
  expect(json.tags).toEqual(["a", "b"]);

  // JSON invalide -> doit lever
  expect(() => api.jsonToProtobuf("not json", descriptor, "user.User")).toThrow();

  // Type de message inconnu -> doit lever
  expect(() => api.jsonToProtobuf('{"id":"x"}', descriptor, "user.Nope")).toThrow();
}
```

```ts
// tests/conversion.node.test.ts — cible Node natif
import { test } from "vitest";
import * as protoruf from "@protoruf/node";
import { runConversionSuite } from "./conversion.shared";

test("conversion parity (node)", () => runConversionSuite(protoruf));
```

```ts
// tests/conversion.wasm.test.ts — cible WASM (init asynchrone)
import { beforeAll, test } from "vitest";
import init, * as protoruf from "@protoruf/wasm";
import { runConversionSuite } from "./conversion.shared";

beforeAll(async () => { await init(); });   // charge et instancie le module .wasm
test("conversion parity (wasm)", () => runConversionSuite(protoruf));
```

### Ce qu'il faut couvrir

- **Parité avec `core.rs`** : round-trip, **maps**, **enums** (nom ↔ numéro), **oneof**, champs
  répétés, valeurs par défaut — pour garantir une sémantique identique à Python.
- **`compileProtoFromSources`** : compilation multi-fichiers (imports résolus en mémoire) et
  import d'un type *well-known* (`google/protobuf/timestamp.proto`).
- **Le piège des entiers 64 bits** (cf. §9) : un test explicite documentant le comportement choisi
  (perte de précision `number` vs `BigInt`).
- **`DescriptorCache`** : équivalence de sortie avec les fonctions libres + réutilisation sur
  plusieurs messages.
- **Spécifique WASM** : init asynchrone, et passage `Uint8Array` ↔ `Uint8Array` à la frontière.

### Intégration CI

Ajouter un job `test` **en amont du `publish`** dans `publish-npm.yml` (ainsi qu'un workflow de
CI sur PR), par ex. :

```yaml
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: npm ci
      - run: npx @napi-rs/cli build --release            # build natif local
      - run: wasm-pack build --release --target web --out-dir pkg -- --features wasm
      - run: npx vitest run                               # exécute les 2 projets (node + wasm)
```

> Le job `publish` doit alors dépendre de `test` (`needs: [build-native, build-wasm, test]`) pour
> ne **jamais publier un paquet dont les tests échouent**.

---

## 9. Pièges à anticiper

- **Entiers 64 bits.** `core.rs` sérialise avec `stringify_64_bit_integers(false)` : les
  `int64`/`uint64` sortent en **nombres JSON**. En JavaScript, `JSON.parse` les charge en
  `number` (double) et **perd la précision au-delà de 2^53**. Si des grands entiers existent,
  prévoir soit un parseur JSON gérant les `BigInt`, soit basculer la sérialisation côté Rust
  sur `stringify_64_bit_integers(true)` pour la cible JS.
- **`Uint8Array` vs `Buffer`.** Sous Node/napi, un `Buffer` *est* un `Uint8Array` ; viser
  `Uint8Array` dans les types garde le code portable navigateur.
- **Coût de chargement WASM.** Le module `.wasm` s'initialise de façon asynchrone (`await
  init()`) côté web ; en tenir compte dans l'API publique (init paresseuse).
- **Réutiliser `DescriptorCache`.** Comme en Python, c'est le levier perf n°1 : décoder le
  pool une fois et le réutiliser évite le coût dominant de `DescriptorPool::decode` à chaque
  appel.
- **Parité de tests.** Rejouer les cas de `core.rs` (round-trip, maps, enums, oneof) au niveau
  JS pour garantir une sémantique identique à Python.

---

## 10. Plan d'action proposé

1. **Ajouter `compile_proto_from_sources(contents, root)` dans `core.rs`** — fonction **nouvelle
   et séparée** (la fonction `compile_proto` basée sur un chemin reste inchangée). C'est le seul
   changement nécessaire dans le cœur, et il bénéficie aux trois cibles.
2. **Ajouter une feature `node` + `src/napi.rs`** qui wrappe `core::*` (effort faible, cœur
   intact). C'est la cible la plus proche de l'existant Python.
3. **Générer les types `.d.ts`** et publier un paquet npm `@protoruf/node` via napi-rs, en
   réutilisant la matrice CI des wheels.
4. **Cible navigateur** : feature `wasm` + `src/wasm.rs`. La compilation y passe **obligatoirement
   par `compile_proto_from_sources` (chaîne de caractères)** — `compile_proto(path)` n'y est pas
   exposé. Les descripteurs déjà compilés (`load_descriptor`/bytes) restent une alternative.
5. **Couche TS idiomatique** (camelCase + helpers Zod) miroir des helpers Python.
6. **Suite de tests Vitest** (cf. §8) : corpus partagé exécuté contre Node et WASM, en parité
   avec les cas de `core.rs`.
7. **CI/CD** : ajouter `publish-npm.yml` (cf. §7) — job `test` Vitest, matrice native, job wasm
   unique et job `publish` (dépendant de `test`), calqués sur le workflow PyPI existant.
8. **Documenter** que `compile_proto` (FS) est Node/natif uniquement et que WASM utilise
   `compile_proto_from_sources`.

---

## Annexe — correspondance Python ↔ JS/TS

| Python | JS/TS (napi/wasm) | Disponible navigateur ? |
|---|---|---|
| `compile_proto(path, include_paths)` | `compileProto(path, includePaths?)` | ❌ (FS) — Node/natif uniquement |
| _(nouveau)_ `compile_proto_from_sources(files, root)` | `compileProtoFromSources(files, root)` | ✅ compilation 100 % en mémoire |
| `load_descriptor(path)` | `loadDescriptor(path)` (Node) / `fetch` → bytes (web) | ⚠️ via fetch |
| `json_to_protobuf(...)` | `jsonToProtobuf(...)` | ✅ |
| `protobuf_to_json(...)` | `protobufToJson(...)` | ✅ |
| `pydantic_to_protobuf(...)` | `objectToProtobuf(...)` (+ Zod) | ✅ |
| `protobuf_to_pydantic(...)` | `protobufToObject(..., schema)` (+ Zod) | ✅ |
| `DescriptorCache` | `DescriptorCache` | ✅ |

---

## Question ouverte — La compilation Protobuf en mémoire dans le navigateur est-elle un vecteur d'attaque ?

Compiler des `.proto` (et décoder des messages) **côté navigateur, à partir d'entrées
potentiellement contrôlées par un attaquant**, mérite une analyse de sécurité avant mise en
production. Réponse courte : **la surface d'attaque est réelle mais largement contenue** — le
risque crédible est un **déni de service côté client (l'onglet)**, pas une exécution de code
arbitraire ni une exfiltration de données.

### Ce qui protège (le contexte est plutôt favorable)

- **Sûreté mémoire de Rust.** `protox`, `prost` et `prost-reflect` sont du Rust *safe*
  (`protox` est même `#![deny(unsafe_code)]`). Les classes d'attaques mémoire classiques
  (buffer overflow, use-after-free → RCE) ne s'appliquent pas comme en C/C++.
- **Bac à sable WASM du navigateur.** Un module WASM s'exécute dans une mémoire linéaire isolée,
  **sans accès au système de fichiers, au réseau, ni au DOM** par défaut. Même nourri d'une entrée
  malveillante, le code ne peut ni lire des fichiers locaux, ni exfiltrer des données, ni
  s'échapper vers l'hôte. Le « rayon de souffle » se limite à l'onglet courant.
- **Pas de `protoc` ni de plugins.** Contrairement à une chaîne `protoc` classique, il n'y a **ni
  exécution de plugin externe, ni génération de code** : `protox` parse et produit un descriptor
  set en mémoire. Aucune exécution de code issu du `.proto`.

### Les risques qui subsistent (essentiellement du DoS client)

1. **Épuisement CPU/mémoire à la compilation.** Un `.proto` pathologique (gigantesque, profondément
   imbriqué, milliers de types/champs) peut faire consommer beaucoup de CPU/mémoire au parser.
   Exécuté sur le **thread principal**, cela **fige l'onglet**.
2. **« Bombes » de décodage à la conversion.** Le wire-format Protobuf permet des facteurs
   d'amplification (messages très imbriqués, champs répétés massifs, `length-delimited` mentant sur
   leur taille). Décoder des **bytes attaquants** via `protobufToJson` peut provoquer une forte
   allocation mémoire ou une **récursion profonde** (risque de débordement de pile WASM).
3. **JSON hostile.** `jsonToProtobuf` parse du JSON arbitraire : profondeur d'imbrication,
   clés/chaînes énormes — mêmes risques d'épuisement que tout parseur JSON.
4. **Descripteur non fiable.** `DescriptorPool::decode` / `DescriptorCache` décodent aussi des
   bytes ; un descriptor set falsifié provenant d'une source non vérifiée est une entrée à traiter
   comme hostile au même titre que le reste.
5. **Chaîne d'approvisionnement.** Le binaire `.wasm` lui-même : s'assurer de son **intégrité**
   (SRI / hash épinglé) et **maintenir les dépendances à jour** (`protox`, `prost*`) pour absorber
   les correctifs de robustesse du parser.

### Mitigations recommandées

- **Exécuter dans un Web Worker** (voire un worker dédié, jetable) plutôt que sur le thread
  principal : un blocage ou un crash mémoire n'y gèle pas l'UI, et le worker peut être *terminé*
  avec un **timeout**.
- **Borne de temps / budget** : annuler la compilation ou la conversion au-delà d'un seuil
  (watchdog côté worker).
- **Traiter toute entrée externe comme hostile** : `.proto`, bytes Protobuf, JSON *et* descriptor.
  Valider la sortie avec **Zod** (`protobufToObject`) plutôt que de faire confiance à la structure.
- **Préférer, quand c'est possible, des descripteurs pré-compilés** (côté serveur/CI) et **ne pas
  exposer `compileProtoFromSources` à des sources arbitraires** : ne compiler du `.proto`
  utilisateur que si le cas d'usage l'exige réellement (éditeur en ligne, etc.).
- **Limiter la taille des entrées** en amont (quotas sur la longueur du `.proto`, du JSON, des
  bytes).
- **Intégrité du module** : SRI sur le `.wasm`, audit des dépendances (`cargo audit`,
  `npm audit`) dans la CI (§7).

### Mise en perspective : WASM vs natif (napi)

Paradoxalement, **la cible navigateur (WASM) est la plus sûre des deux** : le code y tourne en bac
à sable, sans FS ni réseau. La cible **Node natif (napi)** s'exécute, elle, **avec les privilèges du
processus** et un **accès complet au système de fichiers** ; une faille de robustesse y a un impact
potentiellement bien plus grave. Côté serveur, les bonnes pratiques habituelles s'appliquent donc
d'autant plus : entrées non fiables isolées, limites de ressources, dépendances tenues à jour.

> **En résumé.** Oui, c'est une surface d'attaque à prendre au sérieux dès que les entrées sont
> contrôlées par un tiers — mais grâce à la sûreté mémoire de Rust et au bac à sable WASM, le
> scénario réaliste est un **DoS de l'onglet**, pas une compromission. Un **Web Worker avec
> timeout + des limites de taille + des dépendances à jour** ramènent ce risque à un niveau
> acceptable pour la plupart des usages.
