# API Python

Documentation complète de l'API Python de `protoruf`.

## Vue d'ensemble

La bibliothèque expose 3 fonctions principales:

```python
from protoruf import (
    compile_proto,      # Compiler un fichier .proto
    json_to_protobuf,   # Convertir JSON → Protobuf
    protobuf_to_json    # Convertir Protobuf → JSON
)
```

---

## Référence API

### `compile_proto(proto_path, include_paths=None, output_path=None)`

Compile un fichier `.proto` en un descriptor set binaire.

**Paramètres:**

| Nom | Type | Description |
|-----|------|-------------|
| `proto_path` | `str` \| `Path` | Chemin vers le fichier `.proto` |
| `include_paths` | `list[str]` \| `None` | Chemins de recherche pour les imports (par défaut: parent du proto) |
| `output_path` | `str` \| `Path` \| `None` | Chemin de sauvegarde du descriptor (optionnel) |

**Retourne:** `bytes` - Descriptor set sérialisé

**Exemple:**

```python
from protoruf import compile_proto

# Compilation simple
descriptor = compile_proto("schema.proto")

# Avec include paths personnalisés
descriptor = compile_proto(
    "proto/api/message.proto",
    include_paths=["proto", "vendor/proto"]
)

# Sauvegarder le descriptor
descriptor = compile_proto(
    "schema.proto",
    output_path="schema.desc"
)

# Charger un descriptor existant
from protoruf import load_descriptor
descriptor = load_descriptor("schema.desc")
```

**Exceptions:**

- `RuntimeError` - Échec de la compilation (fichier introuvable, erreur de syntaxe)

---

### `json_to_protobuf(json_str, descriptor_bytes, message_type=None)`

Convertit une chaîne JSON en message Protobuf binaire.

**Paramètres:**

| Nom | Type | Description |
|-----|------|-------------|
| `json_str` | `str` | Données JSON à convertir |
| `descriptor_bytes` | `bytes` | Descriptor set de `compile_proto` |
| `message_type` | `str` \| `None` | Nom complet du message (ex: `package.Message`) |

**Retourne:** `bytes` - Message Protobuf encodé

**Exemple:**

```python
from protoruf import compile_proto, json_to_protobuf

descriptor = compile_proto("user.proto")

# Conversion basique
json_data = '{"id": "123", "name": "Alice"}'
protobuf_bytes = json_to_protobuf(
    json_data,
    descriptor,
    message_type="user.User"
)

# Avec des données complexes
json_data = '''
{
    "id": "usr_001",
    "email": "alice@example.com",
    "role": "ROLE_ADMIN",
    "permissions": ["read", "write"],
    "profile": {
        "avatar": "https://..."
    }
}
'''
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="user.User")
```

**Exceptions:**

- `ValueError` - JSON invalide ou type de message introuvable
- `TypeError` - Type de donnée incompatible avec le schema

---

### `protobuf_to_json(protobuf_bytes, descriptor_bytes, pretty=False, message_type=None)`

Convertit un message Protobuf binaire en chaîne JSON.

**Paramètres:**

| Nom | Type | Description |
|-----|------|-------------|
| `protobuf_bytes` | `bytes` | Données Protobuf à décoder |
| `descriptor_bytes` | `bytes` | Descriptor set de `compile_proto` |
| `pretty` | `bool` | Formattage JSON avec indentation (par défaut: `False`) |
| `message_type` | `str` \| `None` | Nom complet du message (ex: `package.Message`) |

**Retourne:** `str` - Données JSON

**Exemple:**

```python
from protoruf import compile_proto, protobuf_to_json

descriptor = compile_proto("user.proto")

# Conversion basique
json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="user.User"
)

# Avec formatage pretty
json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="user.User",
    pretty=True
)

print(json_str)
# {
#   "id": "usr_001",
#   "email": "alice@example.com",
#   ...
# }
```

**Exceptions:**

- `RuntimeError` - Échec du décodage Protobuf
- `ValueError` - Type de message introuvable

---

### `load_descriptor(descriptor_path)`

Charge un descriptor set depuis un fichier.

**Paramètres:**

| Nom | Type | Description |
|-----|------|-------------|
| `descriptor_path` | `str` \| `Path` | Chemin vers le fichier `.desc` |

**Retourne:** `bytes` - Descriptor set

**Exemple:**

```python
from protoruf import load_descriptor, json_to_protobuf

descriptor = load_descriptor("schema.desc")
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="Message")
```

---

## Types de données supportés

### Types scalaires

| JSON | Protobuf | Exemple |
|------|----------|---------|
| `string` | `string` | `"hello"` |
| `number` | `int32`, `int64`, `uint32`, `uint64` | `42` |
| `number` | `float`, `double` | `3.14` |
| `boolean` | `bool` | `true` |

### Enums

Les enums peuvent être spécifiés par nom ou par numéro:

```python
# Par nom (recommandé)
json_data = '{"role": "ROLE_ADMIN"}'

# Par numéro
json_data = '{"role": 1}'
```

À la conversion Protobuf → JSON, les enums sont retournés en format numérique.

### Messages imbriqués

```python
json_data = '''
{
    "user": {
        "id": "123",
        "profile": {
            "name": "Alice",
            "settings": {
                "theme": "dark"
            }
        }
    }
}
'''
```

### Listes (champs répétés)

```python
json_data = '''
{
    "tags": ["python", "rust", "protobuf"],
    "scores": [95, 87, 92]
}
'''
```

### Maps (dictionnaires)

```python
json_data = '''
{
    "metadata": {
        "author": "Alice",
        "version": "1.0",
        "tags": "a,b,c"
    }
}
'''
```

### Oneof fields

```python
# Un seul champ du oneof doit être présent
json_data = '''
{
    "data": {
        "temperature": {
            "celsius": 23.5
        }
    }
}
'''

# ou
json_data = '''
{
    "data": {
        "humidity": {
            "relative": 55.2
        }
    }
}
'''
```

---

## Intégration Pydantic

`protoruf` s'intègre parfaitement avec Pydantic pour la validation:

```python
from pydantic import BaseModel, Field
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json
import json

class User(BaseModel):
    id: str = Field(..., pattern=r"^usr_[a-z]+$")
    email: str
    role: str = "user"

# Compiler le proto
descriptor = compile_proto("user.proto")

# Créer et valider avec Pydantic
user = User(id="usr_alice", email="alice@example.com")

# Convertir en JSON
json_data = user.model_dump_json()

# Convertir en Protobuf
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="user.User")

# Round-trip
result_json = protobuf_to_json(protobuf_bytes, descriptor, message_type="user.User")
result_data = json.loads(result_json)
validated_user = User(**result_data)
```

---

## Bonnes pratiques

### 1. Réutiliser les descriptors

```python
# ❌ Mauvais - compiler à chaque fois
for data in items:
    descriptor = compile_proto("schema.proto")
    protobuf = json_to_protobuf(data, descriptor)

# ✅ Bon - compiler une fois
descriptor = compile_proto("schema.proto")
for data in items:
    protobuf = json_to_protobuf(data, descriptor)
```

### 2. Sauvegarder les descriptors

```python
# Compiler et sauvegarder
descriptor = compile_proto("schema.proto", output_path="schema.desc")

# Charger ultérieurement
descriptor = load_descriptor("schema.desc")
```

### 3. Gérer les erreurs

```python
from protoruf import compile_proto, json_to_protobuf

try:
    descriptor = compile_proto("schema.proto")
except RuntimeError as e:
    print(f"Erreur de compilation: {e}")

try:
    protobuf = json_to_protobuf(json_data, descriptor, message_type="Message")
except (ValueError, TypeError) as e:
    print(f"Erreur de conversion: {e}")
```

### 4. Spécifier le message type

Toujours spécifier `message_type` pour éviter les ambiguïtés:

```python
# ❌ Ambigu
protobuf = json_to_protobuf(data, descriptor)

# ✅ Explicite
protobuf = json_to_protobuf(data, descriptor, message_type="package.Message")
```

---

## Performance

### Comparaison JSON vs Protobuf

```python
import json
from protoruf import compile_proto, json_to_protobuf

descriptor = compile_proto("schema.proto")
json_data = json.dumps({"id": "123", "name": "Test", "value": 42})

# Taille JSON
json_size = len(json_data.encode('utf-8'))

# Taille Protobuf
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="Message")
protobuf_size = len(protobuf_bytes)

print(f"JSON: {json_size} bytes")
print(f"Protobuf: {protobuf_size} bytes")
print(f"Compression: {(1 - protobuf_size/json_size) * 100:.1f}%")
```

Typiquement, Protobuf réduit la taille de 50-80% par rapport à JSON.

---

## Exemples complets

Voir le dossier [examples/](../examples/) pour des cas d'usage complets:

- `01_basic_user_example.py` - Utilisation basique
- `02_ecommerce_example.py` - Système e-commerce
- `03_iot_sensors_example.py` - Capteurs IoT avec oneof
- `04_pydantic_integration.py` - Intégration Pydantic

---

## Prochaines étapes

- [Documentation Rust](rust.md) - Implémentation interne
- [Guide d'installation](setup.md) - Configuration
- [Exemples](../examples/README.md) - Cas d'usage
