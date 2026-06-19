# Analyse : compiler protoruf et l'interfacer avec C / C++

Cette note analyse comment **réutiliser le cœur Rust de `protoruf` pour exposer la même
interface en C et en C++** que celle disponible aujourd'hui en Python (et étudiée pour
[JavaScript/TypeScript](javascript-typescript.md)).

Comme pour les autres cibles, l'objectif n'est pas de réécrire la logique de conversion mais
de **rajouter une couche de binding** — ici une **ABI C** (`extern "C"`) — au-dessus du code
déjà existant, exactement comme PyO3 le fait pour Python.

---

## Sommaire

1. [Point de départ : une architecture déjà favorable](#1-point-de-départ--une-architecture-déjà-favorable)
2. [La cible : une ABI C consommable par C et C++](#2-la-cible--une-abi-c-consommable-par-c-et-c)
3. [Le point dur : franchir la frontière FFI (mémoire & erreurs)](#3-le-point-dur--franchir-la-frontière-ffi-mémoire--erreurs)
4. [Mise en œuvre concrète](#4-mise-en-œuvre-concrète)
5. [Interface C cible (header généré)](#5-interface-c-cible-header-généré)
6. [Surcouche C++ idiomatique (RAII)](#6-surcouche-c-idiomatique-raii)
7. [Build & distribution (CMake / pkg-config)](#7-build--distribution-cmake--pkg-config)
8. [CI/CD : produire les bibliothèques par plateforme](#8-cicd--produire-les-bibliothèques-par-plateforme)
9. [Tests](#9-tests)
10. [Pièges à anticiper](#10-pièges-à-anticiper)
11. [Plan d'action proposé](#11-plan-daction-proposé)
12. [Annexe — correspondance Python ↔ C/C++](#annexe--correspondance-python--ccpp)
13. [Question ouverte — vecteur d'attaque cyber ?](#question-ouverte--exposer-protoruf-en-c-est-il-un-vecteur-dattaque-)

---

## 1. Point de départ : une architecture déjà favorable

Le rappel est le même que pour les autres bindings : le projet sépare proprement le **cœur
métier** de la **couche de liaison**.

| Fichier | Rôle | Dépendances |
|---|---|---|
| `src/core.rs` | Logique métier **pure Rust** : compilation `.proto`, JSON ↔ Protobuf, descriptor pool | `protox`, `prost`, `prost-reflect`, `serde_json` |
| `src/lib.rs` | Fine couche de **binding PyO3** | `pyo3` |

`core.rs` ne dépend pas de Python. Toutes les fonctions exposées sont des _wrappers_ autour de
fonctions `core::*` qui renvoient des `Result<_, String>` :

```rust
// core.rs — signatures réutilisées telles quelles par TOUS les bindings
pub fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> Result<Vec<u8>, String>;
pub fn json_to_protobuf_bytes(json_str: &str, descriptor_bytes: &[u8], message_type: &str) -> Result<Vec<u8>, String>;
pub fn protobuf_to_json_string(protobuf_bytes: &[u8], descriptor_bytes: &[u8], pretty: bool, message_type: &str) -> Result<String, String>;
pub fn load_descriptor_pool(descriptor_bytes: &[u8]) -> Result<DescriptorPool, String>;
pub fn get_message_descriptor(pool: &DescriptorPool, message_type: &str) -> Result<MessageDescriptor, String>;
pub fn json_to_protobuf_bytes_with_descriptor(json_str: &str, desc: &MessageDescriptor) -> Result<Vec<u8>, String>;
pub fn protobuf_to_json_string_with_descriptor(protobuf_bytes: &[u8], desc: &MessageDescriptor, pretty: bool) -> Result<String, String>;
```

> **Conséquence :** exposer C/C++ revient, là encore, à écrire un module de binding
> (`src/capi.rs`) qui appelle les mêmes `core::*`. Le cœur est partagé, jamais dupliqué.

### Surface d'API à reproduire

| Symbole Python | Équivalent C visé | Notes |
|---|---|---|
| `compile_proto(path, include_paths)` | `protoruf_compile_proto(...)` | accès disque — OK en C natif |
| `compile_proto_from_sources(files, root)` *(cf. doc JS/TS)* | `protoruf_compile_proto_from_sources(...)` | compilation en mémoire, utile aussi en C embarqué/sandbox |
| `load_descriptor(path)` | code C trivial (`fread`) | lecture de fichier |
| `json_to_protobuf(...)` | `protoruf_json_to_protobuf(...)` | |
| `protobuf_to_json(...)` | `protoruf_protobuf_to_json(...)` | |
| `DescriptorCache` | handle opaque `ProtorufDescriptorCache*` | objet à durée de vie gérée par l'appelant |
| `pydantic_*` | — | spécifique Python ; en C/C++ c'est du glue applicatif au-dessus du JSON |

---

## 2. La cible : une ABI C consommable par C et C++

Il n'existe qu'**une seule** voie de binding bas-niveau portable : exposer une **interface
`extern "C"`** (ABI C), c'est-à-dire un ensemble de fonctions libres sans *name mangling*,
manipulant uniquement des types compatibles C (pointeurs, entiers, `size_t`).

C'est volontairement **« C, pas C++ »** côté ABI :

- **C** consomme directement le header généré.
- **C++** consomme la **même** ABI C (dans un `extern "C" { ... }`), puis on lui fournit une
  **surcouche RAII** optionnelle (§6) pour une ergonomie idiomatique (`std::vector`,
  `std::string`, exceptions).

> Exposer directement des types C++ (classes, `std::string`, templates) à travers une frontière
> de bibliothèque est **fragile** (l'ABI C++ n'est pas stable entre compilateurs/versions). La
> règle d'or du FFI : **l'ABI est en C**, le confort C++ se construit *par-dessus*, côté en-tête.

### Outillage

- **`extern "C"` + `#[no_mangle]`** côté Rust pour figer les symboles.
- **[`cbindgen`](https://github.com/mozilla/cbindgen)** pour **générer automatiquement le header
  `.h`** (et un `.hpp` avec namespace pour C++) à partir des signatures Rust.
- **`crate-type`** : le `Cargo.toml` déclare déjà `["cdylib", "rlib"]`. On ajoute **`staticlib`**
  pour permettre l'édition de liens statique (`.a`).

```toml
[lib]
crate-type = ["cdylib", "staticlib", "rlib"]
```

---

## 3. Le point dur : franchir la frontière FFI (mémoire & erreurs)

C'est **le** sujet central de cette étude — l'équivalent, pour C/C++, de ce qu'était le système
de fichiers pour le navigateur. L'ABI C ne connaît ni `Result`, ni `String`, ni `Vec`, ni
`Option`. Trois problèmes à résoudre proprement :

### 3.1 Renvoyer des tampons d'octets : qui possède la mémoire ?

`core::*` produit des `Vec<u8>` (protobuf) et des `String` (JSON), **alloués par l'allocateur de
Rust**. C ne doit **jamais** les libérer avec son `free()` : l'allocateur peut différer →
*undefined behavior*. La convention retenue :

- Rust alloue le tampon, en transfère la propriété à l'appelant via un pointeur + une longueur
  (out-params).
- L'appelant **rend** la mémoire à Rust via une fonction de libération dédiée
  **`protoruf_free_buffer(ptr, len)`**.

```c
/* Toute mémoire renvoyée par protoruf DOIT être libérée par protoruf, jamais par free(). */
ProtorufStatus protoruf_json_to_protobuf(
    const char* json_str,
    const uint8_t* descriptor_bytes, size_t descriptor_len,
    const char* message_type,
    uint8_t** out_buf, size_t* out_len);   /* tampon possédé par protoruf */

void protoruf_free_buffer(uint8_t* buf, size_t len);
```

### 3.2 Reporter les erreurs sans exceptions ni `PyErr`

Pas d'exceptions à travers l'ABI C. On combine **un code de statut** + **un message d'erreur
récupérable**. Deux conventions classiques ; on privilégie un **last-error par thread** (simple
pour l'appelant, sans out-param d'erreur partout) :

```c
typedef enum {
    PROTORUF_OK = 0,
    PROTORUF_ERR_INVALID_JSON = 1,
    PROTORUF_ERR_MESSAGE_NOT_FOUND = 2,
    PROTORUF_ERR_DECODE = 3,
    PROTORUF_ERR_COMPILE = 4,
    PROTORUF_ERR_NULL_ARG = 5,
    PROTORUF_ERR_INTERNAL = 99,
} ProtorufStatus;

/* Message détaillé de la dernière erreur sur CE thread (chaîne possédée par protoruf). */
const char* protoruf_last_error(void);
```

Côté Rust, chaque `Err(String)` de `core::*` est stocké dans une variable *thread-local* et
traduit en `ProtorufStatus`.

### 3.3 `panic` Rust ≠ exception C

Un `panic!` qui traverserait la frontière FFI est un *undefined behavior*. **Chaque fonction
exportée enveloppe son corps dans `std::panic::catch_unwind`** et convertit un panic en
`PROTORUF_ERR_INTERNAL` (même esprit que la note du `Cargo.toml` : ne pas mettre
`panic = "abort"`, mais ici on capture explicitement au niveau FFI).

> Tous les pointeurs entrants sont **vérifiés non-`NULL`** (→ `PROTORUF_ERR_NULL_ARG`), et la
> doc précise les contrats (durée de vie, encodage UTF-8 des `const char*`, propriété des
> tampons). C'est le prix de l'absence de garde-fou du langage hôte.

---

## 4. Mise en œuvre concrète

### 4.1 Module de binding (`src/capi.rs`)

Comme `lib.rs`/napi/wasm, on ne fait que **traduire types & erreurs** autour de `core::*`,
derrière un *feature flag* pour ne pas embarquer PyO3.

```rust
// src/capi.rs  (feature "capi")
use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;
use crate::core;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

fn set_last_error(msg: String) {
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::new(msg).ok());
}

#[repr(C)]
pub enum ProtorufStatus { Ok = 0, InvalidJson = 1, MessageNotFound = 2, Decode = 3, Compile = 4, NullArg = 5, Internal = 99 }

#[no_mangle]
pub extern "C" fn protoruf_last_error() -> *const c_char {
    LAST_ERROR.with(|e| match &*e.borrow() {
        Some(s) => s.as_ptr(),
        None => std::ptr::null(),
    })
}

/// Libère un tampon précédemment renvoyé par protoruf (jamais avec free()).
#[no_mangle]
pub extern "C" fn protoruf_free_buffer(buf: *mut u8, len: usize) {
    if !buf.is_null() {
        unsafe { drop(Vec::from_raw_parts(buf, len, len)); }
    }
}

/// JSON -> Protobuf. `out_buf`/`out_len` reçoivent un tampon possédé par protoruf.
#[no_mangle]
pub extern "C" fn protoruf_json_to_protobuf(
    json_str: *const c_char,
    descriptor_bytes: *const u8, descriptor_len: usize,
    message_type: *const c_char,
    out_buf: *mut *mut u8, out_len: *mut usize,
) -> ProtorufStatus {
    let result = std::panic::catch_unwind(|| {
        if json_str.is_null() || descriptor_bytes.is_null() || message_type.is_null()
            || out_buf.is_null() || out_len.is_null() {
            return ProtorufStatus::NullArg;
        }
        let json = unsafe { CStr::from_ptr(json_str) }.to_str().unwrap_or("");
        let desc = unsafe { std::slice::from_raw_parts(descriptor_bytes, descriptor_len) };
        let mtype = unsafe { CStr::from_ptr(message_type) }.to_str().unwrap_or("");

        match core::json_to_protobuf_bytes(json, desc, mtype) {
            Ok(mut v) => {
                v.shrink_to_fit();
                let (ptr, len) = (v.as_mut_ptr(), v.len());
                std::mem::forget(v);               // propriété transférée à l'appelant
                unsafe { *out_buf = ptr; *out_len = len; }
                ProtorufStatus::Ok
            }
            Err(e) => { set_last_error(e); ProtorufStatus::InvalidJson }
        }
    });
    result.unwrap_or_else(|_| { set_last_error("panic in protoruf".into()); ProtorufStatus::Internal })
}
```

`protobuf_to_json`, `compile_proto`, `compile_proto_from_sources` suivent le même patron.

### 4.2 `DescriptorCache` → handle opaque

L'équivalent de la classe Python : un type **opaque** côté C, dont l'appelant gère la durée de
vie via *create/destroy*.

```rust
#[no_mangle]
pub extern "C" fn protoruf_descriptor_cache_new(
    descriptor_bytes: *const u8, len: usize, out_cache: *mut *mut DescriptorCache,
) -> ProtorufStatus { /* Box::into_raw(...) */ }

#[no_mangle]
pub extern "C" fn protoruf_descriptor_cache_json_to_protobuf(
    cache: *mut DescriptorCache, json_str: *const c_char, message_type: *const c_char,
    out_buf: *mut *mut u8, out_len: *mut usize,
) -> ProtorufStatus { /* (*cache).resolve(...) + core::*_with_descriptor */ }

/// Détruit un cache créé par protoruf_descriptor_cache_new.
#[no_mangle]
pub extern "C" fn protoruf_descriptor_cache_free(cache: *mut DescriptorCache) {
    if !cache.is_null() { unsafe { drop(Box::from_raw(cache)); } }
}
```

> La logique interne du cache (pool `prost-reflect` + mémoïsation des `MessageDescriptor`) est
> **identique** à celle de `lib.rs` ; seul l'emballage change (`Box::into_raw`/`from_raw` au lieu
> de `#[pyclass]`).

### 4.3 Compilation en mémoire (utile aussi en C)

La fonction `compile_proto_from_sources(files, root)` proposée dans la
[doc JS/TS](javascript-typescript.md#4-le-point-dur--compile_proto-et-le-système-de-fichiers)
(ajout **séparé** dans `core.rs`, sans toucher à `compile_proto`) sert aussi C/C++ : utile pour
les environnements **sans système de fichiers** (firmware, sandbox) ou pour compiler des `.proto`
reçus en mémoire. Côté ABI, on passe deux tableaux parallèles (noms + contenus) :

```c
ProtorufStatus protoruf_compile_proto_from_sources(
    const char* const* file_names, const char* const* file_contents, size_t file_count,
    const char* root,
    uint8_t** out_buf, size_t* out_len);
```

---

## 5. Interface C cible (header généré)

`cbindgen` produit ce header (forme visée, miroir de l'API Python). Toutes les fonctions
renvoient un `ProtorufStatus` ; les sorties passent par out-params ; **la mémoire renvoyée
appartient à protoruf**.

```c
/* protoruf.h — généré par cbindgen */
#ifndef PROTORUF_H
#define PROTORUF_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum ProtorufStatus {
  PROTORUF_OK = 0,
  PROTORUF_ERR_INVALID_JSON = 1,
  PROTORUF_ERR_MESSAGE_NOT_FOUND = 2,
  PROTORUF_ERR_DECODE = 3,
  PROTORUF_ERR_COMPILE = 4,
  PROTORUF_ERR_NULL_ARG = 5,
  PROTORUF_ERR_INTERNAL = 99
} ProtorufStatus;

typedef struct ProtorufDescriptorCache ProtorufDescriptorCache; /* opaque */

/* Message détaillé de la dernière erreur sur le thread courant (ne pas libérer). */
const char* protoruf_last_error(void);

/* Libère un tampon renvoyé par protoruf. NE PAS utiliser free(). */
void protoruf_free_buffer(uint8_t* buf, size_t len);

/* Compile un .proto depuis le disque (Node/natif : OK ; pas d'accès disque -> voir _from_sources). */
ProtorufStatus protoruf_compile_proto(
    const char* proto_path,
    const char* const* include_paths, size_t include_count,
    uint8_t** out_buf, size_t* out_len);

/* Compile des .proto fournis en mémoire (aucun accès disque). */
ProtorufStatus protoruf_compile_proto_from_sources(
    const char* const* file_names, const char* const* file_contents, size_t file_count,
    const char* root,
    uint8_t** out_buf, size_t* out_len);

/* JSON (UTF-8) -> Protobuf. */
ProtorufStatus protoruf_json_to_protobuf(
    const char* json_str,
    const uint8_t* descriptor_bytes, size_t descriptor_len,
    const char* message_type,
    uint8_t** out_buf, size_t* out_len);

/* Protobuf -> JSON (UTF-8, terminé par \0 dans out_buf). */
ProtorufStatus protoruf_protobuf_to_json(
    const uint8_t* protobuf_bytes, size_t protobuf_len,
    const uint8_t* descriptor_bytes, size_t descriptor_len,
    bool pretty,
    const char* message_type,
    char** out_buf, size_t* out_len);

/* --- Descriptor cache (pool pré-décodé, réutilisable) --- */
ProtorufStatus protoruf_descriptor_cache_new(
    const uint8_t* descriptor_bytes, size_t descriptor_len,
    ProtorufDescriptorCache** out_cache);

ProtorufStatus protoruf_descriptor_cache_json_to_protobuf(
    ProtorufDescriptorCache* cache, const char* json_str, const char* message_type,
    uint8_t** out_buf, size_t* out_len);

ProtorufStatus protoruf_descriptor_cache_protobuf_to_json(
    ProtorufDescriptorCache* cache, const uint8_t* protobuf_bytes, size_t protobuf_len,
    const char* message_type, bool pretty,
    char** out_buf, size_t* out_len);

void protoruf_descriptor_cache_free(ProtorufDescriptorCache* cache);

#ifdef __cplusplus
} /* extern "C" */
#endif
#endif /* PROTORUF_H */
```

### Exemple d'utilisation en C

```c
#include "protoruf.h"
#include <stdio.h>

int main(void) {
    uint8_t* desc; size_t desc_len;
    if (protoruf_compile_proto("schema.proto", NULL, 0, &desc, &desc_len) != PROTORUF_OK) {
        fprintf(stderr, "compile: %s\n", protoruf_last_error());
        return 1;
    }

    uint8_t* pb; size_t pb_len;
    ProtorufStatus st = protoruf_json_to_protobuf(
        "{\"id\":\"123\"}", desc, desc_len, "user.User", &pb, &pb_len);
    if (st != PROTORUF_OK) { fprintf(stderr, "%s\n", protoruf_last_error()); return 1; }

    /* ... utiliser pb / pb_len ... */

    protoruf_free_buffer(pb, pb_len);
    protoruf_free_buffer(desc, desc_len);
    return 0;
}
```

---

## 6. Surcouche C++ idiomatique (RAII)

Au-dessus de la **même ABI C**, un en-tête C++ *header-only* offre une ergonomie proche de
Pydantic/Python : `std::vector<uint8_t>`, `std::string`, exceptions, et **libération
automatique** (RAII) — l'appelant n'a plus à penser à `protoruf_free_*`.

```cpp
// protoruf.hpp — header-only, au-dessus de protoruf.h
#pragma once
#include "protoruf.h"
#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>

namespace protoruf {

class Error : public std::runtime_error {
public:
    explicit Error(ProtorufStatus st)
        : std::runtime_error(protoruf_last_error() ? protoruf_last_error() : "protoruf error"),
          status(st) {}
    ProtorufStatus status;
};

inline std::vector<uint8_t> json_to_protobuf(
    const std::string& json, const std::vector<uint8_t>& descriptor, const std::string& message_type) {
    uint8_t* out = nullptr; size_t len = 0;
    auto st = protoruf_json_to_protobuf(
        json.c_str(), descriptor.data(), descriptor.size(), message_type.c_str(), &out, &len);
    if (st != PROTORUF_OK) throw Error(st);
    std::vector<uint8_t> result(out, out + len);   // copie dans un conteneur C++
    protoruf_free_buffer(out, len);                 // puis on rend la mémoire à Rust
    return result;
}

// RAII autour du handle opaque — équivalent de la classe DescriptorCache de Python.
class DescriptorCache {
public:
    explicit DescriptorCache(const std::vector<uint8_t>& descriptor) {
        if (protoruf_descriptor_cache_new(descriptor.data(), descriptor.size(), &h_) != PROTORUF_OK)
            throw Error(PROTORUF_ERR_INTERNAL);
    }
    ~DescriptorCache() { protoruf_descriptor_cache_free(h_); }
    DescriptorCache(const DescriptorCache&) = delete;            // non copiable
    DescriptorCache& operator=(const DescriptorCache&) = delete;

    std::vector<uint8_t> json_to_protobuf(const std::string& json, const std::string& message_type) {
        uint8_t* out = nullptr; size_t len = 0;
        auto st = protoruf_descriptor_cache_json_to_protobuf(h_, json.c_str(), message_type.c_str(), &out, &len);
        if (st != PROTORUF_OK) throw Error(st);
        std::vector<uint8_t> r(out, out + len);
        protoruf_free_buffer(out, len);
        return r;
    }
private:
    ProtorufDescriptorCache* h_ = nullptr;
};

} // namespace protoruf
```

> Cette surcouche est **100 % côté en-tête** (pas de nouveau code Rust) : elle se contente
> d'emballer l'ABI C, comme `compiler.py` / `pydantic_*` sont du Python pur au-dessus du binding.

---

## 7. Build & distribution (CMake / pkg-config)

### Artefacts produits

| Artefact | Origine | Usage |
|---|---|---|
| `libprotoruf.so` / `.dylib` / `protoruf.dll` | `crate-type = cdylib` | édition de liens dynamique |
| `libprotoruf.a` / `protoruf.lib` | `crate-type = staticlib` | édition de liens statique |
| `protoruf.h` (+ `protoruf.hpp`) | `cbindgen` | en-têtes |
| `protoruf.pc` | gabarit | intégration `pkg-config` |

### Génération du header

```bash
cargo install cbindgen
cbindgen --config cbindgen.toml --crate protoruf --output include/protoruf.h
```

```toml
# cbindgen.toml
language = "C"
include_guard = "PROTORUF_H"
[export]
prefix = "Protoruf"        # types
item_types = ["enums", "structs", "functions", "opaque"]
```

### Intégration CMake côté consommateur

```cmake
# find_package / pkg-config, puis :
add_executable(app main.c)
target_include_directories(app PRIVATE /usr/local/include)
target_link_libraries(app PRIVATE protoruf)   # libprotoruf.so/.a
```

> **Soname / versionnage.** Pour la bibliothèque dynamique, fixer un `soname` et **respecter la
> compatibilité ABI** : on ne modifie pas la signature d'une fonction exportée existante. Toute
> évolution incompatible passe par un bump de version majeure (cf. semver côté C).

---

## 8. CI/CD : produire les bibliothèques par plateforme

Même logique que pour les wheels PyPI (`.github/workflows/publish-pypi.yml`) et le futur
`publish-npm.yml` : une **matrice OS/arch** car une bibliothèque native est **spécifique à
OS+arch**, suivie d'un job qui empaquette.

| Étape | Détail |
|---|---|
| Matrice | `linux-x86_64`, `linux-aarch64`, `macos-x86_64`, `macos-arm64`, `windows-x86_64` |
| Build | `cargo build --release --features capi` (ou `--profile dist`) → `.so`/`.dylib`/`.dll` + `.a` |
| Header | `cbindgen` une fois (indépendant de la plateforme) |
| Empaquetage | archive `protoruf-<version>-<os>-<arch>.{tar.gz,zip}` contenant `lib/`, `include/`, `protoruf.pc` |
| Publication | **GitHub Releases** (assets), éventuellement Conan / vcpkg |

```yaml
# extrait .github/workflows/release-c.yml
jobs:
  build:
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
        with: { targets: ${{ matrix.target }} }
      - run: cargo build --release --features capi --target ${{ matrix.target }}
      - run: cbindgen --config cbindgen.toml --crate protoruf --output include/protoruf.h
      - uses: actions/upload-artifact@v4
        with: { name: protoruf-${{ matrix.target }}, path: "target/${{ matrix.target }}/release/*protoruf*" }
```

> **Différence clé avec PyPI/npm** : il n'y a **pas de registre standard universel** pour le C.
> Le canal naturel est **GitHub Releases** (archives par plateforme + header), avec, en option,
> une recette **Conan** ou **vcpkg** pour les écosystèmes C++ modernes. Le déclencheur reste le
> même (`on: release: [published]`).

---

## 9. Tests

Comme `core.rs` est couvert par `cargo test` et les bindings Python par `pytest`, **la couche C
doit avoir ses propres tests**, à deux niveaux :

1. **Tests Rust du module FFI** (`#[cfg(test)]` dans `capi.rs`) : appeler les fonctions
   `extern "C"` depuis Rust et vérifier codes de statut, propriété mémoire (pas de fuite),
   gestion du `NULL`, et capture de panic. C'est le plus simple à automatiser.
2. **Tests d'intégration côté C/C++** : compiler un petit programme contre la bibliothèque
   produite et rejouer la **parité avec `core.rs`** (round-trip, maps, enums, oneof, valeurs par
   défaut, JSON invalide, type inconnu).
   - **C** : un harnais minimal (assertions) ou **Unity/Check**.
   - **C++** : **Catch2** ou **GoogleTest** pour exercer la surcouche RAII et vérifier que les
     exceptions sont bien levées.

Outils complémentaires fortement recommandés vu le caractère « sans garde-fou » du FFI :

- **Valgrind** / **ASan/LeakSanitizer** sur les tests C pour traquer fuites et accès invalides à
  la frontière (mémoire rendue, double-free, `free()` au lieu de `protoruf_free_buffer`).
- **Miri** sur les tests Rust du module FFI pour détecter les *undefined behaviors* (manipulation
  de pointeurs bruts).

### Intégration CI

Ajouter un job `test` **en amont du job de publication** (cf. §8) :

```yaml
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --features capi                 # cœur + module FFI
      - run: cmake -S tests/c -B build && cmake --build build && ctest --test-dir build
      # option: re-lancer les tests C sous ASan/Valgrind
```

> Le job de publication doit dépendre de `test` (`needs: [test, build]`) pour ne jamais publier
> une archive dont les tests échouent.

---

## 10. Pièges à anticiper

- **Propriété de la mémoire.** *Le* piège n°1. Toute mémoire renvoyée par protoruf se libère via
  `protoruf_free_buffer` / `protoruf_descriptor_cache_free`, **jamais** avec `free()`/`delete`.
  À marteler dans la doc du header et à encapsuler par la surcouche C++ (RAII).
- **Panics traversant la FFI.** Sans `catch_unwind` à chaque fonction exportée, un panic Rust =
  *undefined behavior* côté C. Non négociable.
- **Validité & encodage des `const char*`.** Pointeurs non-`NULL` et **UTF-8 valide** (les `&str`
  de Rust l'exigent) ; définir le comportement si l'entrée ne l'est pas.
- **Entiers 64 bits.** Comme pour JS, `core.rs` sérialise avec `stringify_64_bit_integers(false)`
  → les `int64/uint64` sortent en **nombres JSON**. En C/C++, attention au parseur JSON
  consommateur (précision). Documenter, ou exposer une option de stringification.
- **Sécurité des threads.** `protoruf_last_error()` est **par thread** (thread-local) : ne pas
  partager le pointeur renvoyé entre threads. Le handle `DescriptorCache` est conçu pour des
  lectures concurrentes (RwLock interne, cf. `lib.rs`), mais sa **destruction** doit être
  exclusive.
- **Stabilité ABI.** Ne jamais changer la signature d'une fonction exportée publiée : versionner.
- **Taille du binaire / dépendances.** La `cdylib` embarque tout l'arbre Rust ; surveiller la
  taille et utiliser `--profile dist` (LTO, strip) pour les artefacts publiés.

---

## 11. Plan d'action proposé

1. **Ajouter `compile_proto_from_sources` dans `core.rs`** (mutualisé avec la cible JS/TS) —
   fonction nouvelle et séparée, `compile_proto` inchangée.
2. **Ajouter une feature `capi` + `src/capi.rs`** : fonctions `extern "C"` autour de `core::*`,
   gestion `ProtorufStatus` + last-error thread-local + `catch_unwind` + `protoruf_free_buffer`.
3. **`DescriptorCache` en handle opaque** (`Box::into_raw`/`from_raw`), miroir de la classe
   Python.
4. **Générer `protoruf.h`** via `cbindgen` (+ `cbindgen.toml`), et fournir `protoruf.hpp` (RAII)
   pour C++.
5. **Packaging** : ajouter `staticlib` au `crate-type`, gabarits `protoruf.pc` / CMake, archives
   par plateforme.
6. **Tests** : tests Rust du module FFI (+ Miri) et tests d'intégration C/C++ (Catch2/Unity, +
   ASan/Valgrind), en parité avec `core.rs`.
7. **CI/CD** : workflow `release-c.yml` — job `test`, matrice OS/arch, publication sur GitHub
   Releases (option Conan/vcpkg), calqué sur le workflow PyPI.
8. **Documenter** les contrats FFI (propriété mémoire, UTF-8, durée de vie des handles,
   thread-safety).

---

## Annexe — correspondance Python ↔ C/C++

| Python | C (ABI) | C++ (surcouche RAII) |
|---|---|---|
| `compile_proto(path, include_paths)` | `protoruf_compile_proto(...)` | `protoruf::compile_proto(path, includes)` |
| `compile_proto_from_sources(files, root)` | `protoruf_compile_proto_from_sources(...)` | `protoruf::compile_proto_from_sources(...)` |
| `load_descriptor(path)` | `fread` applicatif | `std::ifstream` applicatif |
| `json_to_protobuf(...)` | `protoruf_json_to_protobuf(...)` | `protoruf::json_to_protobuf(...)` |
| `protobuf_to_json(...)` | `protoruf_protobuf_to_json(...)` | `protoruf::protobuf_to_json(...)` |
| `DescriptorCache(...)` | `protoruf_descriptor_cache_new/...//_free` | `protoruf::DescriptorCache` (RAII) |
| `pydantic_to_protobuf(...)` | — (glue applicatif) | — (au-dessus du JSON) |
| _(gestion erreurs)_ `raise ValueError/RuntimeError` | `ProtorufStatus` + `protoruf_last_error()` | `throw protoruf::Error` |
| _(libération mémoire)_ GC Python | `protoruf_free_buffer(...)` | automatique (RAII) |

---

## Question ouverte — Exposer protoruf en C est-il un vecteur d'attaque ?

À l'inverse de la cible navigateur (WASM, fortement bac-à-sablée — cf.
[doc JS/TS](javascript-typescript.md#question-ouverte--la-compilation-protobuf-en-mémoire-dans-le-navigateur-est-elle-un-vecteur-dattaque-)),
**la cible C/C++ est la plus exposée des trois**, et mérite donc l'analyse la plus prudente.

### Ce qui protège encore

- **Le cœur reste du Rust *safe*.** La logique de parsing/conversion (`protox`, `prost`,
  `prost-reflect`) conserve ses garanties mémoire : pas de buffer overflow *à l'intérieur* du
  traitement. Une entrée hostile (proto/protobuf/JSON malformé) est gérée par des `Result`.

### Ce qui change radicalement par rapport au navigateur

- **Aucun bac à sable.** Le code s'exécute **dans le processus hôte**, avec ses **privilèges** et
  un **accès complet au système de fichiers et au réseau**. Une faille a un impact bien supérieur
  à un simple onglet figé.
- **La frontière FFI introduit elle-même des risques** que le langage ne couvre plus :
  - **Mauvaise gestion mémoire côté appelant** : `free()` au lieu de `protoruf_free_buffer`,
    double-free, oubli de libération (fuite), usage d'un tampon après libération. Ce sont des
    failles **du code C consommateur**, mais induites par le contrat FFI.
  - **`const char*` non terminé / non-UTF-8** passé par l'appelant → comportement indéfini si
    mal géré côté Rust (d'où la validation systématique).
  - **Panic non capturé** traversant la frontière = *undefined behavior* (d'où `catch_unwind`).
- **DoS applicatif** : comme partout, des entrées pathologiques (proto géant, « bombes » de
  décodage protobuf très imbriquées, JSON profond) peuvent épuiser CPU/mémoire — mais **ici cela
  peut faire tomber un service serveur entier**, pas juste un onglet.

### Mitigations recommandées

- **Encapsuler systématiquement via la surcouche C++ RAII** (ou un wrapper maison en C) pour
  éliminer par construction les erreurs de propriété mémoire.
- **Traiter toute entrée externe comme hostile** (proto, descriptor, bytes, JSON) ; valider la
  sortie (schéma applicatif).
- **Limiter les ressources** : tailles d'entrée bornées, et pour les charges non fiables,
  isoler le traitement (processus dédié, `seccomp`/conteneur, *resource limits*).
- **Outillage mémoire en CI** : ASan/Valgrind/LeakSanitizer sur les tests C, Miri sur le module
  FFI (cf. §9) — la première ligne de défense contre les bugs de frontière.
- **Maintenir les dépendances à jour** (`cargo audit`) pour absorber les correctifs de robustesse
  des parseurs, et **figer l'ABI** pour éviter les surprises au *linking*.

> **En résumé.** Le *cœur* de protoruf garde la sûreté mémoire de Rust, mais **l'ABI C supprime
> le filet de sécurité côté consommateur** et s'exécute **sans bac à sable, avec les privilèges du
> processus**. Le risque dominant n'est pas une faille *dans* protoruf mais une **mauvaise
> utilisation de la frontière FFI** (gestion mémoire) et un **DoS** sur des entrées hostiles. Une
> surcouche RAII, des entrées bornées/isolées et l'outillage mémoire en CI ramènent ce risque à un
> niveau maîtrisé — en gardant à l'esprit que cette cible est intrinsèquement moins protégée que
> WASM.
