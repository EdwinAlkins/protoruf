# Analyse : compiler protoruf et l'interfacer avec Java

Cette note analyse comment **réutiliser le cœur Rust de `protoruf` pour exposer la même
interface en Java** que celle disponible aujourd'hui en Python (et étudiée pour
[JavaScript/TypeScript](javascript-typescript.md) et [C/C++](c-cpp.md)).

Comme pour les autres cibles, l'objectif n'est pas de réécrire la logique de conversion mais
de **rajouter une couche de binding** au-dessus du code existant. La nouveauté ici : il existe
un outil, **[BoltFFI](https://boltffi.dev/)**, qui **génère automatiquement** le binding Java
*et* le pont JNI à partir d'annotations Rust — exactement le rôle que joue `maturin` pour
Python, `napi-rs` pour Node ou `wasm-pack` pour le navigateur.

---

## Sommaire

1. [Point de départ : une architecture déjà favorable](#1-point-de-départ--une-architecture-déjà-favorable)
2. [Deux voies : BoltFFI ou l'ABI C via Panama/FFM](#2-deux-voies--boltffi-ou-labi-c-via-panamaffm)
3. [Mise en œuvre avec BoltFFI](#3-mise-en-œuvre-avec-boltffi)
4. [Le point dur : pont JNI, cycle de vie des handles, int64](#4-le-point-dur--pont-jni-cycle-de-vie-des-handles-int64)
5. [Interface Java cible](#5-interface-java-cible)
6. [Correspondance des types Rust ↔ Java](#6-correspondance-des-types-rust--java)
7. [Build & distribution (Maven / Gradle)](#7-build--distribution-maven--gradle)
8. [CI/CD : produire un JAR multi-plateforme](#8-cicd--produire-un-jar-multi-plateforme)
9. [Tests (JUnit)](#9-tests-junit)
10. [Pièges à anticiper](#10-pièges-à-anticiper)
11. [Plan d'action proposé](#11-plan-daction-proposé)
12. [Annexe — correspondance Python ↔ Java](#annexe--correspondance-python--java)
13. [Question ouverte — vecteur d'attaque cyber ?](#question-ouverte--exposer-protoruf-en-java-est-il-un-vecteur-dattaque-)

---

## 1. Point de départ : une architecture déjà favorable

Même rappel que pour les autres bindings : `core.rs` est du **Rust pur** réutilisable, `lib.rs`
n'est qu'une **couche de liaison** PyO3.


| Fichier       | Rôle                                                                                  | Dépendances                                      |
| ------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------ |
| `src/core.rs` | Logique métier **pure Rust** : compilation `.proto`, JSON ↔ Protobuf, descriptor pool | `protox`, `prost`, `prost-reflect`, `serde_json` |
| `src/lib.rs`  | Fine couche de **binding PyO3**                                                       | `pyo3`                                           |


Les fonctions de `core.rs` renvoient des `Result<_, String>` et sont **directement
transposables** :

```rust
pub fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> Result<Vec<u8>, String>;
pub fn json_to_protobuf_bytes(json_str: &str, descriptor_bytes: &[u8], message_type: &str) -> Result<Vec<u8>, String>;
pub fn protobuf_to_json_string(protobuf_bytes: &[u8], descriptor_bytes: &[u8], message_type: &str, pretty: bool) -> Result<String, String>;
// + load_descriptor_pool / get_message_descriptor / *_with_descriptor (cf. DescriptorCache)
```

> **Conséquence :** comme pour Python/Node/WASM/C, exposer Java revient à écrire un module de
> binding (`src/boltffi.rs`) qui appelle les mêmes `core::*`. Le cœur est partagé, jamais
> dupliqué.

### Pourquoi le mapping tombe juste pour protoruf

L'API de protoruf manipule essentiellement des **octets** et des **chaînes** — précisément ce
que les outils FFI Java savent traduire sans friction :


| Type Rust de l'API      | Type Java                               | Adapté à protoruf ?                |
| ----------------------- | --------------------------------------- | ---------------------------------- |
| `Vec<u8>` / `&[u8]`     | `byte[]`                                | ✅ descripteurs & messages protobuf |
| `String` / `&str`       | `String`                                | ✅ JSON, chemins, noms de message   |
| `bool`                  | `boolean`                               | ✅ `pretty`                         |
| `Result<T, String>`     | méthode qui *lève* une exception        | ✅ erreurs                          |
| objet à méthodes (impl) | objet **opaque** (handle géré par Rust) | ✅ `DescriptorCache`                |


---

## 2. Deux voies : BoltFFI ou l'ABI C via Panama/FFM

Deux approches sérieuses existent pour relier Rust et Java. Elles ne s'excluent pas, mais
n'impliquent pas le même effort.

### Option A — BoltFFI (recommandée) — *génère le binding*

[BoltFFI](https://boltffi.dev/) est un générateur de bindings multi-langages pour Rust
(Swift, Kotlin, **Java**, TypeScript/WASM). On **annote** le code Rust (`#[export]` sur les
fonctions, `#[data]` sur les types valeur, `#[error]` sur les types d'erreur) puis
`boltffi pack java` produit **les sources Java + la bibliothèque JNI** prêtes à l'emploi.

- ✅ **Tout est généré** : pas de JNI manuel, pas de gestion mémoire manuelle.
- ✅ **API Java idiomatique** : `byte[]`, `String`, exceptions, *records* (Java 16+) ou classes
finales (Java 8+).
- ✅ **Performant** : conçu pour minimiser le coût de la frontière (zéro-copie quand c'est
possible) ; le projet revendique des bindings *jusqu'à ~1000× plus rapides qu'UniFFI*.
- ✅ **Cohérent avec l'existant** : même philosophie « un outil → un paquet » que maturin/napi.
- ⚠️ Outil plus récent que JNI/JNA → suivre sa maturité et figer la version utilisée.
- ⚠️ Nécessite **un compilateur C** au build (génération du pont JNI) et un **JDK 8+**.

### Option B — ABI C + Project Panama / FFM API — *réutilise le travail C*

Java 22+ propose la **Foreign Function & Memory API** (`java.lang.foreign`) et l'outil
`**jextract`**, qui consomment **directement une ABI C**. Or l'[étude C/C++](c-cpp.md) définit
déjà cette ABI (`protoruf.h`). On peut donc **réutiliser le même `cdylib` + header** et générer
le binding Java avec `jextract`, sans JNI.

- ✅ **Réutilise l'ABI C** déjà conçue (un seul artefact natif pour C, C++ *et* Java).
- ✅ Pas de couche JNI (FFM appelle le C directement).
- ❌ **JDK 22+ requis** (FFM stabilisée tardivement).
- ❌ API générée plus bas-niveau (`MemorySegment`, arènes) → demande une surcouche Java « jolie »
écrite à la main (gestion des `byte[]`/`String`, libération via `protoruf_free_buffer`).

### Pour mémoire — JNI brut / JNA

- `**jni` crate (JNI manuel)** : maximal en contrôle, mais beaucoup de *boilerplate* et code
spécifique JNI à écrire dans Rust. C'est ce que BoltFFI génère pour nous.
- **JNA** : très simple à câbler sur une ABI C, mais lent (réflexion à l'appel) — peu adapté à
un chemin haut-débit comme protoruf.

### Recommandation

- **BoltFFI** comme voie principale : c'est l'équivalent Java de maturin/napi/wasm-pack — binding
idiomatique généré, compatible JDK 8+, packaging multi-plateforme.
- **Panama/FFM** si l'on veut **mutualiser strictement l'ABI C** avec C/C++ et qu'on peut
imposer **JDK 22+**.

Le reste de cette note suit la voie **BoltFFI**.

---

## 3. Mise en œuvre avec BoltFFI

### 3.1 Module de binding (`src/boltffi.rs`)

Comme `lib.rs`/napi/wasm/capi, on ne fait que **traduire types & erreurs** autour de `core::`*,
derrière un *feature flag*.

```rust
// src/boltffi.rs  (feature "java")
use boltffi::{export, error};
use crate::core;

/// Type d'erreur exposé : devient une exception Java (try/catch).
#[error]
pub enum ProtorufError {
    InvalidJson(String),
    MessageNotFound(String),
    Decode(String),
    Compile(String),
}

#[export]
pub fn compile_proto(proto_path: String, include_paths: Option<Vec<String>>) -> Result<Vec<u8>, ProtorufError> {
    core::compile_proto(&proto_path, include_paths).map_err(ProtorufError::Compile)
}

#[export]
pub fn compile_proto_from_sources(file_names: Vec<String>, file_contents: Vec<String>, root: String)
    -> Result<Vec<u8>, ProtorufError>
{
    let files = file_names.into_iter().zip(file_contents).collect();
    core::compile_proto_from_sources(files, &root).map_err(ProtorufError::Compile)
}

#[export]
pub fn json_to_protobuf(json_str: String, descriptor_bytes: Vec<u8>, message_type: String)
    -> Result<Vec<u8>, ProtorufError>
{
    core::json_to_protobuf_bytes(&json_str, &descriptor_bytes, &message_type)
        .map_err(ProtorufError::InvalidJson)
}

#[export]
pub fn protobuf_to_json(protobuf_bytes: Vec<u8>, descriptor_bytes: Vec<u8>, message_type: String, pretty: bool)
    -> Result<String, ProtorufError>
{
    core::protobuf_to_json_string(&protobuf_bytes, &descriptor_bytes, &message_type, pretty)
        .map_err(ProtorufError::Decode)
}
```

On retrouve **les mêmes appels `core::*`** que partout ailleurs ; seul l'emballage
(`#[export]`, `ProtorufError`) change.

### 3.2 `DescriptorCache` → objet opaque

`DescriptorCache` est un objet **à méthodes** : avec BoltFFI, un `#[export] impl` sur une struct
en fait un **objet opaque** côté Java (Java détient un *handle*, l'état vit côté Rust). La
logique interne (pool `prost-reflect` + mémoïsation des `MessageDescriptor`) est **identique** à
`lib.rs` ; seules les annotations changent.

```rust
pub struct DescriptorCache { /* pool + cache de descripteurs, comme dans lib.rs */ }

#[export]
impl DescriptorCache {
    #[export]
    pub fn new(descriptor_bytes: Vec<u8>) -> Result<DescriptorCache, ProtorufError> { /* ... */ }

    #[export]
    pub fn json_to_protobuf(&self, json_str: String, message_type: String) -> Result<Vec<u8>, ProtorufError> {
        let desc = self.resolve(&message_type)?;
        core::json_to_protobuf_bytes_with_descriptor(&json_str, &desc).map_err(ProtorufError::InvalidJson)
    }

    #[export]
    pub fn protobuf_to_json(&self, protobuf_bytes: Vec<u8>, message_type: String, pretty: bool)
        -> Result<String, ProtorufError>
    {
        let desc = self.resolve(&message_type)?;
        core::protobuf_to_json_string_with_descriptor(&protobuf_bytes, &desc, pretty).map_err(ProtorufError::Decode)
    }
}
```

### 3.3 Génération

```bash
cargo install boltffi_cli
boltffi init                 # crée boltffi.toml
boltffi pack java            # génère les sources Java + la bibliothèque JNI
boltffi pack java --release  # build optimisé pour la distribution
```

```toml
# boltffi.toml (extrait)
[java]
package = "com.edwinalkins.protoruf"
min_version = 16             # records modernes ; mettre 8 pour des classes finales
```

---

## 4. Le point dur : pont JNI, cycle de vie des handles, int64

Pour Java, BoltFFI **gère lui-même la mémoire et le marshalling** : le « point dur » n'est donc
plus la frontière mémoire brute (comme en [C/C++](c-cpp.md)) mais trois sujets propres à la JVM.

### 4.1 Pont JNI = compilateur C au build

BoltFFI génère un pont JNI en C, qu'il faut **compiler par plateforme** (un compilateur C est
requis). La conséquence concrète est côté **packaging** : la bibliothèque native
(`.so`/`.dll`/`.dylib`) doit être **embarquée dans le JAR** et chargée au démarrage
(`System.loadLibrary` / extraction d'une ressource). C'est ce que gère le code généré, mais cela
impose une **CI matricielle** (§8).

### 4.2 Cycle de vie des objets opaques (`DescriptorCache`)

Un `DescriptorCache` détient de la **mémoire native** qui **n'est pas libérée par le GC** de
façon déterministe. Il faut donc exposer une libération explicite. Le binding généré expose
typiquement un `close()` ; on l'utilise via `**AutoCloseable` + try-with-resources**, ou un
`java.lang.ref.Cleaner` en filet de sécurité :

```java
try (DescriptorCache cache = new DescriptorCache(descriptor)) {
    byte[] pb = cache.jsonToProtobuf("{\"id\":\"123\"}", "user.User");
}   // mémoire native libérée ici, de façon déterministe
```

> Ne **pas** se reposer sur la finalisation par le GC pour relâcher la mémoire native : sous
> charge, cela provoque une croissance mémoire hors-tas (« off-heap »).

### 4.3 Entiers 64 bits (signés en Java) + précision JSON

Deux nuances :

- `**u64` → `long` (signé).** BoltFFI mappe les entiers non signés sur le type signé le plus
large : un `u64 > 2^63` apparaît **négatif** en Java. Sans incidence sur l'API actuelle de
protoruf (qui échange des `byte[]`/`String`), mais à connaître si l'on expose un jour des
champs numériques directement.
- **Précision JSON.** Comme ailleurs, `core.rs` sérialise avec `stringify_64_bit_integers(false)`
→ les `int64/uint64` sortent en **nombres JSON**. Le **parseur JSON côté Java** (Jackson/Gson)
doit alors mapper vers `long`/`BigInteger` pour ne pas perdre de précision.

---

## 5. Interface Java cible

L'API générée (forme visée, miroir du module Python `protoruf`) :

```java
package com.protoruf;

public final class Protoruf {
    /** Compile un .proto depuis le disque (lève ProtorufException en cas d'échec). */
    public static native byte[] compileProto(String protoPath, String[] includePaths);

    /** Compile des .proto fournis en mémoire (aucun accès disque). */
    public static native byte[] compileProtoFromSources(String[] fileNames, String[] fileContents, String root);

    /** JSON -> Protobuf. */
    public static native byte[] jsonToProtobuf(String jsonStr, byte[] descriptorBytes, String messageType);

    /** Protobuf -> JSON. */
    public static native String protobufToJson(byte[] protobufBytes, byte[] descriptorBytes, String messageType, boolean pretty);
}

/** Pool de descripteurs pré-décodé et réutilisable (objet opaque, à fermer). */
public final class DescriptorCache implements AutoCloseable {
    public DescriptorCache(byte[] descriptorBytes);
    public byte[] jsonToProtobuf(String jsonStr, String messageType);
    public String protobufToJson(byte[] protobufBytes, String messageType, boolean pretty);
    @Override public void close();   // libère la mémoire native
}

/** Erreur de conversion/compilation (RuntimeException). */
public class ProtorufException extends RuntimeException { /* ... */ }
```

### Exemple d'utilisation

```java
import com.protoruf.*;

byte[] descriptor = Protoruf.compileProto("schema.proto", null);

byte[] pb = Protoruf.jsonToProtobuf("{\"id\":\"123\"}", descriptor, "user.User");
String json = Protoruf.protobufToJson(pb, descriptor, "user.User", false);

// Chemin haute-performance : réutiliser le pool décodé
try (DescriptorCache cache = new DescriptorCache(descriptor)) {
    byte[] fast = cache.jsonToProtobuf("{\"id\":\"456\"}", "user.User");
}
```

### 5.1 Au-delà du `String` JSON : une API « objet » idiomatique

Passer du **JSON sous forme de `String`** est simple et reflète l'API Python, mais ce n'est pas
le plus idiomatique côté Java. La bonne nouvelle : **on peut offrir une API à base d'objets
(POJO/record)** — exactement comme `pydantic_to_protobuf` / `protobuf_to_pydantic` le font en
Python par-dessus le binding.

#### Pourquoi garder le JSON `String` *à la frontière FFI*

C'est un choix délibéré, pas une limite :

- **`core.rs` désérialise le JSON directement** en `DynamicMessage` via serde
  (`DynamicMessage::deserialize_with_options`), **sans arbre intermédiaire**. La frontière est
  *naturellement* du JSON.
- **Un objet Java ne peut pas traverser le FFI « tel quel ».** Rust ne sait pas introspecter un
  POJO Java ; et faire marshaller chaque champ par BoltFFI supposerait de **connaître le schéma à
  la compilation** — ce qui irait à l'encontre du principe même de protoruf (dynamique, **sans
  génération de classes**). Il faut donc *toujours* sérialiser l'objet vers quelque chose ; autant
  que ce soit du JSON.
- La vraie question n'est donc pas *quel type traverse le FFI* (ce sera du JSON), mais **où se fait
  la (dé)sérialisation objet ↔ JSON** : mieux vaut une **couche Java ergonomique** (Jackson) que
  d'imposer à l'utilisateur d'appeler `ObjectMapper` lui-même.

#### La couche objet (pur Java, au-dessus du binding)

```java
package com.protoruf;

import com.fasterxml.jackson.databind.ObjectMapper;

/** Helpers objet — équivalent Java de pydantic_to_protobuf / protobuf_to_pydantic. */
public final class ProtorufMapper {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** Objet (POJO/record) -> Protobuf. */
    public static byte[] objectToProtobuf(Object value, byte[] descriptor, String messageType) {
        try {
            String json = MAPPER.writeValueAsString(value);
            return Protoruf.jsonToProtobuf(json, descriptor, messageType);
        } catch (JsonProcessingException e) {
            throw new ProtorufException("sérialisation JSON échouée", e);
        }
    }

    /** Protobuf -> objet typé `T` (validé/mappé par Jackson). */
    public static <T> T protobufToObject(byte[] protobuf, byte[] descriptor, Class<T> type, String messageType) {
        try {
            String json = Protoruf.protobufToJson(protobuf, descriptor, messageType, false);
            return MAPPER.readValue(json, type);      // mapping + validation de forme
        } catch (JsonProcessingException e) {
            throw new ProtorufException("désérialisation JSON échouée", e);
        }
    }
}
```

```java
// Utilisation idiomatique : on manipule des records, plus de JSON à la main
record User(String id, List<String> tags) {}

byte[] pb   = ProtorufMapper.objectToProtobuf(new User("123", List.of("a", "b")), descriptor, "user.User");
User   back = ProtorufMapper.protobufToObject(pb, descriptor, User.class, "user.User");
```

#### Autres formes acceptées (même principe)

- **`com.fasterxml.jackson.databind.JsonNode`** ou **`Map<String, Object>`** : pratiques quand le
  schéma n'est pas mappé à une classe ; sérialisés par Jackson de la même façon.
- **`byte[]` de JSON UTF-8** : micro-optimisation possible pour éviter une conversion
  `String → UTF-8` à la frontière, mais gain marginal et moins lisible — à réserver aux chemins
  ultra-chauds, et à mesurer.

#### À ne pas confondre : les classes protobuf générées (`protobuf-java`)

Si l'on possède déjà une classe `com.google.protobuf.Message` générée par `protoc`, on a
directement `.toByteArray()` / `parseFrom(...)` et **on n'a pas besoin de protoruf**. L'intérêt de
protoruf est précisément le chemin **dynamique, sans `protoc` ni classes générées** : l'API objet
ci-dessus s'appuie sur **vos propres POJO/records**, pas sur des messages protobuf générés.

> En résumé : **l'API `String` JSON reste la primitive** (alignée sur le cœur serde et sur
> Python) ; **l'API objet `ProtorufMapper` est la voie recommandée** pour le code applicatif Java,
> et se construit entièrement *au-dessus* du binding, sans code Rust supplémentaire.

---

## 6. Correspondance des types Rust ↔ Java

Mapping appliqué par BoltFFI (utile pour comprendre l'API générée) :


| Rust                   | Java                                                     | Note                                    |
| ---------------------- | -------------------------------------------------------- | --------------------------------------- |
| `i8 / i16 / i32 / i64` | `byte / short / int / long`                              |                                         |
| `u8 / u16 / u32 / u64` | `short / int / long / long`                              | unsignés élargis ; `**u64` signé**      |
| `f32 / f64`            | `float / double`                                         |                                         |
| `bool`                 | `boolean`                                                |                                         |
| `&str` / `String`      | `String`                                                 | `&str` en paramètre, `String` en retour |
| `Vec<u8>` / `&[u8]`    | `byte[]`                                                 | **binaire** (descripteurs, messages)    |
| `Vec<T>`               | `T[]`                                                    |                                         |
| `Option<T>`            | type *nullable*                                          |                                         |
| `Result<T, E>`         | méthode qui **lève** ; `E` marqué `#[error]` → exception |                                         |
| struct `#[data]`       | *record* (Java 16+) / classe finale                      | type **valeur**, copié                  |
| `impl` exporté         | objet **opaque** (handle géré par Rust)                  | type **référence**                      |


---

## 7. Build & distribution (Maven / Gradle)

### Artefacts produits


| Artefact                                 | Origine                           | Usage                  |
| ---------------------------------------- | --------------------------------- | ---------------------- |
| Sources Java générées (`com.protoruf.`*) | `boltffi pack java`               | compilées dans le JAR  |
| `libprotoruf_jni.so` / `.dll` / `.dylib` | pont JNI compilé (par plateforme) | chargé au runtime      |
| JAR (classes + natifs embarqués)         | Maven/Gradle                      | dépendance applicative |


### Stratégie de packaging multi-plateforme

Une bibliothèque native est **spécifique à OS+arch**. Deux schémas classiques sur la JVM :

1. **« Fat JAR »** : embarquer tous les natifs (`/native/linux-x86_64/…`, `/native/darwin-arm64/…`,
  `/native/windows-x86_64/…`) ; au démarrage, le code extrait/charge celui qui correspond à
   l'hôte. Simple à consommer (un seul artefact).
2. **JAR par plateforme (classifiers)** : un artefact principal + des JAR `*-natives-linux-x86_64`,
  `*-natives-osx-aarch64`, etc. (modèle Netty/LWJGL), tirés via `optionalDependencies`/classifier.
   Plus léger à l'installation.

### Intégration Maven

```xml
<dependency>
  <groupId>com.protoruf</groupId>
  <artifactId>protoruf</artifactId>
  <version>0.1.5</version>
</dependency>
```

> Maven Central impose des contraintes (coordonnées `groupId` vérifiées, **signature GPG**,
> Javadoc & sources). C'est l'équivalent, pour Java, des exigences OIDC de PyPI.

---

## 8. CI/CD : produire un JAR multi-plateforme

Même logique que les wheels PyPI (`.github/workflows/publish-pypi.yml`) : une **matrice OS/arch**
pour compiler le pont JNI, puis un job qui **assemble le JAR** et publie.


| Étape       | Détail                                                                                 |
| ----------- | -------------------------------------------------------------------------------------- |
| Matrice     | `linux-x86_64`, `linux-aarch64`, `macos-x86_64`, `macos-arm64`, `windows-x86_64`       |
| Build natif | `boltffi pack java --release` (compile Rust + pont JNI) → bibliothèque par plateforme  |
| Collecte    | agréger toutes les bibliothèques natives dans `src/main/resources/native/<os>-<arch>/` |
| Assemblage  | `mvn package` (ou Gradle) → JAR contenant classes + natifs                             |
| Publication | **Maven Central** (Sonatype, signature GPG) — option : GitHub Packages                 |


```yaml
# extrait .github/workflows/publish-maven.yml
jobs:
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
        with: { targets: ${{ matrix.target }} }
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - run: cargo install boltffi_cli
      - run: boltffi pack java --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with: { name: native-${{ matrix.target }}, path: target/boltffi/java/native/** }

  publish:
    needs: [build-native, test]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with: { path: src/main/resources/native }
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17, server-id: central }
      - run: mvn --batch-mode deploy   # signe (GPG) et pousse sur Maven Central
        env: { MAVEN_GPG_PASSPHRASE: ${{ secrets.GPG_PASSPHRASE }} }
```

> **Spécificité Java vs PyPI/npm** : un seul **JAR portable** peut embarquer *tous* les natifs
> (fat JAR), contrairement aux wheels/`.node` strictement par plateforme. La matrice sert à
> **produire** les natifs ; le job `publish` les **fusionne** en un artefact unique. Déclencheur
> identique (`on: release: [published]`).

---

## 9. Tests (JUnit)

Comme `core.rs` est couvert par `cargo test` et Python par `pytest`, **le binding Java doit avoir
sa suite JUnit**, en parité avec `core.rs`.

```java
// src/test/java/com/protoruf/ConversionTest.java
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;
import com.protoruf.*;

class ConversionTest {
    static final String PROTO =
        "syntax=\"proto3\"; package user; message User { string id = 1; repeated string tags = 2; }";

    @Test void roundTrip() {
        byte[] desc = Protoruf.compileProtoFromSources(
            new String[]{"user.proto"}, new String[]{PROTO}, "user.proto");
        byte[] pb = Protoruf.jsonToProtobuf("{\"id\":\"123\",\"tags\":[\"a\",\"b\"]}", desc, "user.User");
        String json = Protoruf.protobufToJson(pb, desc, "user.User", false);
        assertTrue(json.contains("\"id\":\"123\""));
    }

    @Test void invalidJsonThrows() {
        byte[] desc = Protoruf.compileProtoFromSources(
            new String[]{"user.proto"}, new String[]{PROTO}, "user.proto");
        assertThrows(ProtorufException.class,
            () -> Protoruf.jsonToProtobuf("not json", desc, "user.User"));
    }

    @Test void cacheReleasesNativeMemory() {
        byte[] desc = Protoruf.compileProtoFromSources(
            new String[]{"user.proto"}, new String[]{PROTO}, "user.proto");
        try (DescriptorCache cache = new DescriptorCache(desc)) {
            assertNotNull(cache.jsonToProtobuf("{\"id\":\"x\"}", "user.User"));
        }   // close() doit libérer sans erreur
    }
}
```

### À couvrir

- **Parité avec `core.rs`** : round-trip, maps, enums (nom ↔ numéro), oneof, valeurs par défaut,
JSON invalide, type de message inconnu.
- `**compileProtoFromSources**` : multi-fichiers (imports en mémoire) + import d'un type
*well-known* Google.
- `**DescriptorCache*`* : équivalence de sortie avec les méthodes statiques, réutilisation sur
plusieurs messages, **libération via `close()`** (pas de fuite hors-tas).
- **Chargement natif multi-plateforme** : test d'intégration vérifiant que la bibliothèque JNI se
charge sur chaque OS de la matrice CI.

### Intégration CI

Job `test` **en amont du `publish`** (`needs: [..., test]`) pour ne jamais publier un JAR dont les
tests échouent — `cargo test` (cœur) **et** `mvn test` (binding) :

```yaml
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - run: cargo test --features java
      - run: boltffi pack java && mvn --batch-mode test
```

---

## 10. Pièges à anticiper

- **Mémoire native non gérée par le GC.** *Le* piège n°1 côté JVM : `DescriptorCache` (et tout
objet opaque) détient de la mémoire hors-tas. Toujours fermer via `**AutoCloseable` /
try-with-resources**, doubler d'un `Cleaner` en filet — jamais s'en remettre à la finalisation.
- **Chargement de la bibliothèque native.** Échecs classiques : natif manquant pour l'OS/arch de
l'hôte, `UnsatisfiedLinkError`, conflit de versions de `glibc` (Linux). Embarquer tous les
natifs (fat JAR) et tester le chargement sur chaque plateforme.
- `**u64` signé / précision int64** (cf. §4.3) : connaître le mapping et configurer le parseur
JSON consommateur.
- **Compilateur C requis au build** du pont JNI : à provisionner dans la CI et documenter pour les
contributeurs qui régénèrent le binding.
- **Threads & JNI.** Les appels traversent JNI ; `DescriptorCache` est conçu pour des lectures
concurrentes (RwLock interne, cf. `lib.rs`), mais sa **fermeture** doit être exclusive (ne pas
`close()` pendant qu'un autre thread l'utilise).
- **Maturité de l'outil.** BoltFFI est récent : **figer sa version**, suivre ses notes de version,
et garder l'option Panama/FFM (§2) comme repli si besoin.
- **Taille du JAR.** Embarquer plusieurs natifs gonfle l'artefact ; envisager les JAR par
classifier si la taille pose problème.

---

## 11. Plan d'action proposé

1. **Ajouter `compile_proto_from_sources` dans `core.rs`** (mutualisé avec JS/TS et C/C++) —
  fonction nouvelle et séparée, `compile_proto` inchangée.
2. **Ajouter une feature `java` + `src/boltffi.rs`** : `#[export]` autour de `core::*`, type
  `#[error] ProtorufError`, `DescriptorCache` en objet opaque.
3. **Configurer `boltffi.toml`** (package `com.protoruf`, `min_version`) et générer via
  `boltffi pack java`.
4. `**DescriptorCache implements AutoCloseable**` côté Java + documentation try-with-resources.
5. **Couche objet `ProtorufMapper`** (pur Java/Jackson, cf. §5.1) : `objectToProtobuf` /
   `protobufToObject(..., Class<T>)` au-dessus du binding — l'API recommandée pour l'applicatif,
   miroir de `pydantic_*`.
6. **Packaging** : choisir fat JAR (tous natifs) ou JAR par classifier ; intégrer Maven/Gradle.
7. **Tests JUnit** en parité avec `core.rs` (+ test de chargement natif multi-plateforme).
8. **CI/CD** : `publish-maven.yml` — job `test`, matrice OS/arch pour les natifs, assemblage du
  JAR, publication Maven Central (signature GPG), calqué sur le workflow PyPI.
9. **Documenter** le cycle de vie des handles, le mapping de types, et le repli Panama/FFM.

---

## Annexe — correspondance Python ↔ Java


| Python                                      | Java (BoltFFI)                                            |
| ------------------------------------------- | --------------------------------------------------------- |
| `compile_proto(path, include_paths)`        | `Protoruf.compileProto(path, includePaths)`               |
| `compile_proto_from_sources(files, root)`   | `Protoruf.compileProtoFromSources(names, contents, root)` |
| `load_descriptor(path)`                     | lecture de fichier applicative (`Files.readAllBytes`)     |
| `json_to_protobuf(...)`                     | `Protoruf.jsonToProtobuf(...)`                            |
| `protobuf_to_json(...)`                     | `Protoruf.protobufToJson(...)`                            |
| `DescriptorCache(...)`                      | `new DescriptorCache(...)` (`AutoCloseable`)              |
| `pydantic_to_protobuf(...)`                 | `ProtorufMapper.objectToProtobuf(obj, ...)` (Jackson, §5.1) |
| `protobuf_to_pydantic(...)`                 | `ProtorufMapper.protobufToObject(bytes, T.class, ...)` (§5.1) |
| *(erreurs)* `raise ValueError/RuntimeError` | `throw ProtorufException`                                 |
| *(libération mémoire)* GC Python            | `close()` / try-with-resources (déterministe)             |


---

## Question ouverte — Exposer protoruf en Java est-il un vecteur d'attaque ?

Le profil de risque de Java se situe **entre** WASM (très bac-à-sablé — cf.
[doc JS/TS](javascript-typescript.md#question-ouverte--la-compilation-protobuf-en-mémoire-dans-le-navigateur-est-elle-un-vecteur-dattaque-))
et C/C++(le plus exposé — cf.
[doc C/C++](c-cpp.md#question-ouverte--exposer-protoruf-en-c-est-il-un-vecteur-dattaque-)).

### Ce qui protège

- **Cœur Rust *safe*.** La logique reste protégée (pas de faille mémoire *dans* le traitement) ;
les entrées hostiles sont gérées par des `Result` → exceptions Java.
- **BoltFFI gère le marshalling.** Contrairement à l'ABI C brute, l'appelant Java **ne manipule
pas de pointeurs** : pas de `free()` manuel, pas de double-free côté consommateur. Les classes
d'erreurs de frontière du C disparaissent largement.

### Ce qui reste à risque

- **Pas de bac à sable.** Le natif s'exécute **dans le processus JVM**, avec ses privilèges et son
accès FS/réseau. Une faille a un impact « serveur », pas « onglet ».
- **Pont JNI.** C'est du C généré : un bug dans cette couche (ou un mésusage du cycle de vie des
handles) peut **crasher la JVM** (pas seulement lever une exception). Un `close()` concurrent à
un usage, ou un usage après `close()`, est une faute applicative dangereuse.
- **Fuite mémoire hors-tas.** Oublier de fermer les `DescriptorCache` provoque une croissance
off-heap invisible pour le monitoring du tas — vecteur de **DoS par épuisement mémoire** sous
charge.
- **DoS applicatif.** Entrées pathologiques (proto géant, « bombes » de décodage protobuf très
imbriquées, JSON profond) → épuisement CPU/mémoire pouvant **faire tomber le service**.

### Mitigations recommandées

- `**AutoCloseable` partout** + `Cleaner` en filet pour la mémoire native ; revue de code ciblée
sur la fermeture des handles.
- **Traiter toute entrée externe comme hostile** (proto, descriptor, bytes, JSON) ; valider la
sortie (schéma applicatif, Jackson/records).
- **Limiter les ressources** : tailles d'entrée bornées ; pour des charges non fiables, isoler
(processus/conteneur dédié, quotas).
- **Maintenir à jour** : `cargo audit` (deps Rust) **et** suivi des versions de BoltFFI et du JDK
(correctifs JNI/sécurité).
- **Tests mémoire/charge en CI** : vérifier l'absence de fuite off-heap (création/fermeture
massives de `DescriptorCache`).

> **En résumé.** BoltFFI **supprime l'essentiel des pièges mémoire** de l'ABI C brute (le
> marshalling est généré et sûr), ce qui rend la cible Java **plus sûre que C/C++** côté
> consommateur. Restent les risques communs aux bindings natifs **non bac-à-sablés** : crash JVM
> via le pont JNI en cas de mésusage des handles, **fuite mémoire hors-tas** si on ne ferme pas
> les objets opaques, et **DoS** sur entrées hostiles. `AutoCloseable` systématique, entrées
> bornées/isolées et dépendances à jour ramènent ce risque à un niveau maîtrisé.

---

## Sources

- [BoltFFI — site & documentation](https://boltffi.dev/)
- [BoltFFI — Getting Started](https://www.boltffi.dev/docs/getting-started)
- [BoltFFI — Types](https://www.boltffi.dev/docs/types)
- [BoltFFI — dépôt GitHub](https://github.com/boltffi/boltffi)
- [BoltFFI — crates.io](https://crates.io/crates/boltffi)
- Référence Java standard pour FFI : [JEP 454 — Foreign Function & Memory API](https://openjdk.org/jeps/454)

