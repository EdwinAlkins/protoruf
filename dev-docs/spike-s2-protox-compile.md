# Spike S2 — protox suffit-il pour compiler les `.proto` de fastgrpc3 ?

Branche : `spike/s2-protox-compile` (depuis `main`). Contexte : conception de fastgrpc3
(`../fastgrpc/docs/conception/`, spike S2 de la feuille de route).

## Question

fastgrpc3 doit compiler des `.proto` sans `protoc`, dans deux situations :

- **proto d'abord** : les `.proto` d'un projet (plusieurs fichiers, imports, standards
  `google/rpc`, health, reflection), avec les commentaires pour les docstrings générées ;
- **code d'abord** : le texte `.proto` généré en mémoire depuis les modèles Pydantic, qui
  peut importer des contrats existants sur disque.

Il faut aussi découper le résultat par fichier pour la reflection gRPC, et des erreurs
lisibles. protox 0.9 (déjà utilisé par protoruf) fournit-il tout cela ?

## Réponse : oui, avec trois compléments à écrire dans protoruf

Les descripteurs produits par protox sont **identiques à ceux de `protoc`** (vérifié sur les
11 fichiers standards). Tout ce qui manque est de l'assemblage côté protoruf, pas une limite
de protox, sauf les *editions* protobuf.

| # | Question | Résultat |
|---|----------|----------|
| Q1 | Plusieurs fichiers racines en une compilation | **oui** : `Compiler::open_files` ; dépendances placées avant les fichiers qui les importent |
| Q2 | Mélanger sources en mémoire, fichiers sur disque et types standards | **oui** : `ChainFileResolver` (mémoire → `third_party/proto` → `GoogleFileResolver`) ; le premier résolveur gagne, donc un projet peut remplacer un fichier fourni |
| Q3 | Commentaires des messages, champs, enums, valeurs, services, méthodes | **oui** : `include_source_info(true)` + `path()` de chaque descripteur prost-reflect, recherché dans le `SourceCodeInfo` du fichier |
| Q4 | Sortie déterministe | **oui** : mêmes entrées, mêmes octets ; l'ordre des racines ne change que l'ordre des fichiers indépendants |
| Q5 | Découpage par fichier et « fichier contenant le symbole » pour la reflection | **oui** : `file_descriptor_proto().encode_to_vec()` redonne exactement l'entrée du `FileDescriptorSet` ; résolution des messages (imbriqués compris), enums, services et méthodes ; dépendances directes via `dependencies()` |
| Q6 | Erreurs localisées | **oui, mais** la position (`fichier:ligne:colonne`) n'est que dans le format `Debug` de l'erreur, pas dans `Display` (voir « Compléments ») |
| Q7 | Fichiers standards hors `google/protobuf` | protox ne fournit que `google/protobuf/*`. Les autres (`google/rpc`, `google/type`, health, reflection) compilent sans problème une fois fournis : ils sont maintenant dans `third_party/proto/` |
| Q8 | Temps de compilation | voir « Mesures » : négligeable pour un service réel |
| Q9 | proto2 et editions | proto2 **lu** (`required`, `[default = …]` visible via `default_value()`) ; editions (`edition = "2023"`) **refusées** par protox 0.9 |
| Q10 | Informations nécessaires à la génération de modèles | **toutes disponibles** : types imbriqués et parent, plages et noms `reserved`, `json_name`, options (`deprecated`), présence, `oneof` réels vs synthétiques (`optional`), maps (types clé/valeur), récursion, forme des méthodes (stream), ordre de déclaration des champs |

Tests : `tests/spike_s2_compile.rs` (12 tests, un par question ; `cargo test --test
spike_s2_compile -- --nocapture` affiche les mesures et les messages d'erreur).

## Équivalence avec `protoc`

Comparaison avec `protoc` (fourni par `grpcio-tools`) sur les 11 fichiers de
`third_party/proto`, fichier par fichier :

- **sans `SourceCodeInfo`** : descripteurs **identiques** (11/11) ;
- **avec `SourceCodeInfo`** : commentaires identiques (11/11) et mêmes positions (11/11) ; seul
  l'ordre des entrées du `SourceCodeInfo` diffère. Équivalent pour tout usage.

À noter : `protox::compile` (utilisé par `compile_proto` aujourd'hui) **garde le
`SourceCodeInfo` par défaut**, contrairement à `protoc`. Les descripteurs actuels de protoruf
sont donc plus gros que nécessaire quand les commentaires ne servent pas.

## Mesures

Schéma synthétique de 200 fichiers × 25 messages × 20 champs (5 000 messages, 100 000
champs), build `--release`, 3 exécutions :

| | compilation | taille du descripteur | décodage du descripteur |
|-|------------:|---------------------:|------------------------:|
| avec commentaires | ~0,43 s | 8,4 Mo | ~0,40 s |
| sans commentaires | ~0,65 s | 2,6 Mo | ~0,11 s |

`google/rpc/error_details.proto` seul : 1,9 Ko sans commentaires, 16,6 Ko avec.

- Garder les commentaires ne ralentit pas la compilation : elle est même plus rapide avec
  (protox les produit puis les retire). Le coût est la taille et le décodage.
- Un service réel (quelques dizaines de messages) se compile en quelques millisecondes : la
  compilation au démarrage du mode code d'abord est acceptable.
- Pour les descripteurs **embarqués dans le code généré**, retirer les commentaires
  (les docstrings sont déjà dans le code Python) : 3 à 9 fois plus petits, décodage 4 fois
  plus rapide.

## Compléments à écrire dans protoruf (version 0.4)

1. **Fichiers standards embarqués** : `third_party/proto/` (copies non modifiées, licence
   Apache 2.0 conservée, voir `third_party/proto/README.md`) servi par un résolveur en mémoire
   (`include_str!`), placé après les sources de l'utilisateur et avant `GoogleFileResolver`.
2. **API de compilation enrichie**, en gardant les fonctions actuelles :

   ```python
   descriptor = protoruf.compile_protos(
       roots=["orders/v1/orders.proto", "orders/v1/admin.proto"],
       include_paths=["proto"],           # disque
       sources={"gen/app.proto": text},   # mémoire (code d'abord), prioritaire
       source_info=False,                 # commentaires : True pour la génération de modèles
       standard_protos=True,              # google/rpc, google/type, health, reflection
   )
   ```

3. **Erreurs avec position** — **fait sur cette branche** : `core.rs` formatait les erreurs
   de protox avec `{}` (`Display`), ce qui donnait `name 'Missing' is not defined`. Avec
   `{:?}` on obtient `e.proto:4:3: name 'Missing' is not defined`, pour `compile_proto` et
   `compile_proto_from_sources`, donc pour les trois bindings. Le préfixe
   `Failed to compile proto file:` ne change pas. Test :
   `core::tests::test_compile_errors_carry_file_line_and_column`.
4. **Introspection et découpage** (E2 et E4 de la conception de fastgrpc3) : tout ce qu'il
   faut est disponible dans prost-reflect (Q5, Q10) ; il reste à l'exposer en Python.

## Limites et points non couverts

- **Editions** protobuf non supportées par protox 0.9 : à surveiller ; pas bloquant tant que
  les contrats sont en proto3.
- Pas de compilation d'un grand dépôt réel (googleapis complet) ; le schéma synthétique
  couvre le volume, pas la variété des options.
- L'équivalence avec `protoc` est vérifiée sur les 11 fichiers standards seulement (pas de
  `protoc` dans la CI de protoruf pour l'étendre).
- `tests/spike_s2_compile.rs` utilise protox directement : ces tests serviront de base aux
  tests de l'API de compilation enrichie, puis pourront être simplifiés.
