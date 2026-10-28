# Spike S1 — codec natif protobuf ↔ objets Python

Branche : `spike/s1-native-codec`. Contexte : conception de fastgrpc.

## Question

Un codec Rust qui lit et construit **directement** les objets Python (sans JSON ni
`DynamicMessage` intermédiaire) est-il nettement plus rapide que le codec de fastgrpc2
(boucle Python par champ + message upb) ? Critère de réussite : **≥ 3×**.

## Réponse : oui

Message `bench.Order` (20 champs, 2 sous-messages, 5 lignes répétées, 4 tags, 394 octets),
Python 3.14.3, AMD Ryzen 7 PRO 7840U, build `--release` (pas le profil `dist`/LTO),
meilleur de 7 × 50 000 opérations, deux exécutions concordantes à < 5 % :

| Codec | encode (µs) | vs natif | decode → modèle validé (µs) | vs natif |
|-------|------------:|---------:|----------------------------:|---------:|
| **natif (ce spike)** | **1,32** | 1× | **5,77** | 1× |
| natif, `decode_dict` sans Pydantic | — | | 2,04 | 0,4× |
| fastgrpc2 0.5.0 | 10,05 | **7,6×** | 18,27 | **3,2×** |
| upb + Pydantic (`model_dump` / `MessageToDict`) | 8,71 | 6,6× | 32,13 | 5,6× |
| protoruf JSON (`DescriptorCache.pydantic_to_protobuf`) | 20,61 | 15,6× | **échouait** (constat 3) ; 14,06 après correctif | 2,4× |
| upb brut, sans Pydantic (référence plancher) | 0,37 | 0,3× | 0,65 | 0,1× |
| `model_validate(dict)` seul | — | | 3,90 | 0,7× |

Message `bench.Small` (2 champs, 7 octets) : natif 0,12 / 0,48 µs ; fastgrpc2 0,68 / 1,44 µs
(5,8× / 3,0×).

Lecture :

- **Encodage** : 7 à 8× plus rapide que fastgrpc2, ~4× au-dessus du plancher upb brut
(qui part d'un message upb déjà construit, pas d'un modèle Pydantic).
- **Décodage** : sur 5,8 µs, environ **2 µs pour construire les dicts** et **3,7 µs de
validation Pydantic**. La validation représente maintenant les deux tiers du coût : le mode
`construct` (sans validation) et l'optimisation `pyo3::ffi` de la construction des dicts
sont les prochains leviers.
- Appeler `__pydantic_validator__.validate_python` depuis Rust coûte moins cher que
`Model.model_validate` (la méthode de classe Python ajoute son propre coût).

## Correction vérifiée

- Octets **identiques à upb** (`google.protobuf` 7.36) pour tous les scalaires et pour
`Order` / `Small` (assertion dans le benchmark et dans `tests/test_native_codec.py`).
- Le codec natif **relit les octets de fastgrpc2** sans erreur.
- `tests/test_native_codec.py` : 17 tests (16 types scalaires, zéros omis puis restitués,
`optional`, `None`, sous-messages, récursion, `repeated` packed et non packed, `map`
scalaire et message, champs inconnus des 4 types de fil, entrées malformées, erreurs
d'encodage nommant le champ, modèles Pydantic, comparaison avec upb).
- `cargo test` : 34 tests verts (dont 6 nouveaux pour `native::wire` et `native::plan`) ;
`cargo check --features node` et sans feature : OK.

## Constats en chemin

1. **fastgrpc2 ne sait pas lire du proto3 standard** quand un champ requis vaut zéro

   (`shipped=False` omis → `Field required`). Attendu : fastgrpc2 met une présence
    explicite partout. Le codec natif, lui, lit les deux formes.
2. **Bug protoruf (chemin JSON)** : `pydantic_to_protobuf` échoue sur un champ `bytes`

   (`invalid base64`). Pydantic sérialise les `bytes` en texte UTF-8 par défaut, le JSON
    protobuf attend du base64. Au décodage, le texte base64 était rendu tel quel comme
    `bytes` (corruption silencieuse). **Corrigé** (`python/protoruf/_pydantic.py`) : les
    modèles contenant des `bytes` passent par `model_dump()` + `to_json(bytes_mode="base64")`
    à l'encodage, et par un décodage base64 guidé par les annotations du modèle au décodage ;
    les autres gardent le chemin JSON direct (surcoût mesuré ~50–75 ns par appel).
3. **Bug protoruf (chemin JSON)** : `protobuf_to_pydantic` échoue sur un champ Pydantic requis

   à valeur nulle, car le JSON produit omet les valeurs par défaut. C'est le « risque n°1 »
    identifié dans l'analyse de fastgrpc2. Le codec natif le résout (valeurs nulles restituées
    au décodage). **Corrigé** sur le chemin JSON aussi : nouvelle option
    `include_default_values` de `protobuf_to_json` (désactivée par défaut, sortie inchangée),
    activée par les helpers Pydantic. Les champs à présence explicite restent absents s'ils
    ne sont pas définis (`None` ne devient pas `0`).

Tests de non-régression : `tests/test_pydantic_regressions.py` (10 tests) et
`core::tests::test_include_default_values_keeps_presence_fields_absent`.

## Compléments : types standards, mode sans validation, enums

### Types standards (well-known types)

Détectés par leur nom complet lors de la construction du plan ; ils ne sont plus décodés
comme des messages génériques.

| Type protobuf | Python | Règles |
|---------------|--------|--------|
| `Timestamp` | `datetime` aware, UTC | un `datetime` avec fuseau est converti en UTC ; un `datetime` naïf est **refusé** sauf `naive_datetime="utc"` ; plage `0001-01-01` → `9999-12-31` vérifiée dans les deux sens ; précision microseconde (nanosecondes tronquées) |
| `Duration` | `timedelta` | `seconds`/`nanos` du même signe ; ±315 576 000 000 s |
| `StringValue`, `Int64Value`… (9 wrappers) | scalaire, ou `None` si absent | `""`/`0` restent distincts de `None` |
| `Struct` / `Value` / `ListValue` | `dict` / valeur JSON / `list` | les nombres sont des `double` : `1` revient `1.0` ; dans un `Value`, `None` = `null_value` |
| `FieldMask` | `list[str]` | |

Calcul de calendrier en Rust pur (`src/native/wkt.rs`, algorithme de H. Hinnant, testé sur
toute la plage). Octets identiques à upb (`FromDatetime`, `FromTimedelta`, `Struct.update`).
Non couverts : `Any` (reste un dict `type_url`/`value`), `Empty` (dict vide, comme un message
ordinaire), `google.type.Date` (absent des fichiers embarqués par protox).

### Mode sans validation : `mode="construct"`

`NativeCodec(desc, "pkg.Msg", Model, mode="construct")` construit directement les instances,
**sous-modèles et membres d'enum compris**, comme `BaseModel.model_construct` :

- les classes des sous-modèles et des enums sont trouvées dans les annotations du modèle
  (`Optional`, `list`, `dict` traversés) par `python/protoruf/_native.py` ;
- les valeurs par défaut des champs absents du message sont appliquées (copie des valeurs
  mutables, `default_factory` appelée à chaque instance) ; un champ requis absent reste non
  défini, comme avec `model_construct` ;
- les champs du message que le modèle ne déclare pas sont ignorés ;
- `model_fields_set`, `__pydantic_extra__` (`extra="allow"`) et l'ordre de `__dict__`
  (donc de `model_dump_json`) sont identiques à `model_construct` ;
- allocation et attributs via l'API C (`tp_alloc`, `PyObject_GenericSetAttr`) ;
- quand les champs du modèle sont exactement ceux du message, dans l'ordre des numéros
  (cas des modèles générés), `__dict__` est construit en une passe ; sinon il est réordonné ;
- repli sur `model_construct` lui-même pour les modèles à `model_post_init`, attributs
  privés, `RootModel` ou `default_factory` lisant les données validées.

### Enums

- `protoruf.OpenIntEnum` : `IntEnum` dont `_missing_` crée un pseudo-membre mis en cache
  `UNRECOGNIZED_<n>` pour une valeur inconnue. Vérifié avec Pydantic (modes Python et JSON) :
  la valeur est conservée et réencodée telle quelle (**spike S3 validé**).
- `unknown_enum="keep"` (défaut, enums proto3 ouverts) ou `"error"` (le codec rejette tout
  numéro non déclaré, dans tous les modes).
- Mode construct : membres d'enum directement (zéro compris pour un champ absent) ; un enum
  fermé (`IntEnum` simple) garde l'`int` pour une valeur inconnue. En mode validé, c'est
  Pydantic qui décide (un `IntEnum` fermé rejette la valeur).
- Encodage depuis un `int`, un `IntEnum`, un `Enum` à valeur entière, ou un **nom proto**
  (`"COLOR_BLUE"`, valeur d'un `StrEnum`, nom de membre), alias compris.

### Mesures (même machine, `bench.Order`, µs par décodage)

| Mode | avant | après |
|------|------:|------:|
| validé (`decode`) | 5,77 | 5,47 |
| construct, sans validation | — | **3,20** |
| `decode_dict` | 2,04 | 1,80 |

Le mode construct est 1,7× plus rapide que le mode validé et 5,7× plus rapide que fastgrpc2
(18,2 µs). Le décodage range désormais les valeurs dans un tableau indexé par champ avant de
construire le dict (une allocation au lieu de trois), d'où le gain sur les autres modes.

Une première version du mode construct, en appels Python (`object.__new__`,
`object.__setattr__`) et avec reconstruction systématique du dict, n'était que 9 % plus
rapide que le mode validé : la validation de pydantic-core coûte à peine plus que
l'assemblage d'une instance par des appels Python.

Tests : `tests/test_native_codec_types.py` (28 tests), `native::wkt` et
`native::plan::plans_well_known_types_and_enums` côté Rust.

## Ce que contient le spike

| Fichier | Rôle |
|---------|------|
| `src/native/wire.rs` | varints, zigzag, fixed32/64, préfixes de longueur, saut de champ inconnu, écriture « longueur réservée puis décalée » sans passe de calcul de taille |
| `src/native/plan.rs` | `CodecPlan` : un plan par message (champs triés par numéro, table d'index directe, `map`, `repeated` packed, présence explicite, récursion), plans d'enums, détection des types standards |
| `src/native/wkt.rs` | calendrier et règles de plage de `Timestamp` / `Duration` |
| `src/python_native.rs` | classe Python `NativeCodec(descriptor, message_type, model=None, *, mode, unknown_enum, naive_datetime)` : `encode`, `decode`, `decode_dict` |
| `python/protoruf/_native.py` | `OpenIntEnum`, introspection des modèles pour le mode construct |
| `tests/test_native_codec.py`, `tests/test_native_codec_types.py` | tests de correction |
| `tests/benchmark/python/benchmark_native_spike.py` + `proto/order.proto` | benchmark comparatif |

Lancer :

```bash
maturin develop --release
python tests/benchmark/python/benchmark_native_spike.py   # fastgrpc2 et protobuf optionnels
pytest tests/test_native_codec.py tests/test_native_codec_types.py
```

## Limites du spike (à traiter avant une version publiée)

- `pyo3::ffi` utilisé seulement pour la construction des instances ; la construction des
dicts et la lecture des champs passent encore par l'API haut niveau de PyO3.
- Pas de liaison explicite numéro → nom Python : le nom Python est supposé égal au nom
proto (les alias Pydantic ne sont pas lus par le codec).
- `oneof` : chaque membre est un champ à présence explicite ; rien n'empêche d'en
encoder deux à la fois.
- Sous-messages répétés pour un même champ singulier : la dernière occurrence gagne au lieu
d'une fusion (écart à la spécification, sans effet sur des émetteurs conformes).
- `float` : la précision est arrondie à f32 à l'encodage comme attendu, mais il n'y a pas de
test sur NaN / infini ni sur `-0.0`.
- Pas de suite de conformité officielle ni de fuzzing.
- API Python non définitive (exposée seulement par `protoruf._protoruf`).
