#!/usr/bin/env python3
"""
Benchmark protoruf vs google.protobuf pour la conversion JSON ↔ Protobuf.

Mesure les temps pour :
  - Sérialisation  : JSON → Protobuf
  - Parsing/Désérialisation : Protobuf → JSON

Utilise le descriptor compilé par protoruf pour les deux bibliothèques,
afin d'éviter toute compilation externe avec protoc.
"""

import time
import json
from pathlib import Path

# --- protoruf -------------------------------------------------
from protoruf import compile_proto, load_descriptor, json_to_protobuf, protobuf_to_json

# --- google.protobuf (optionnel, benchmark uniquement) --------
try:
    from google.protobuf import descriptor_pool, message_factory, json_format
    from google.protobuf.descriptor_pb2 import FileDescriptorSet
    from google.protobuf.internal.python_message import GeneratedProtocolMessageType

    HAS_GOOGLE_PROTOBUF = True
except ImportError:
    HAS_GOOGLE_PROTOBUF = False
    print("⚠️  google.protobuf non installé, seule la partie protoruf sera mesurée.")
    print("   Installez-le avec : uv add protobuf")

# ----------------------------------------------------------------------
# Configuration
# ----------------------------------------------------------------------
PROTO_PATH = Path(__file__).parent / "proto" / "message.proto"
DESC_PATH = Path(__file__).parent / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"

# Nombre d'itérations (à adapter selon votre machine)
ITERATIONS = 100_000  # 100k pour un bench rapide, 1M pour le tableau final


def get_descriptor():
    """Obtient le descriptor compilé (via protoruf)."""
    if not DESC_PATH.exists():
        compile_proto(PROTO_PATH, output_path=DESC_PATH)
    return load_descriptor(DESC_PATH)


def create_google_message_factory(descriptor_bytes: bytes):
    """
    Crée une factory pour instancier des messages google.protobuf dynamiquement
    à partir du FileDescriptorSet compilé par protoruf.

    Returns:
        tuple: (message_class, pool) où message_class est la classe du message
               (ou un callable qui crée une nouvelle instance)
    """
    fds = FileDescriptorSet()
    fds.ParseFromString(descriptor_bytes)

    pool = descriptor_pool.DescriptorPool()
    for file_proto in fds.file:
        pool.Add(file_proto)

    # Récupérer le descripteur du message
    msg_desc = pool.FindMessageTypeByName(MESSAGE_TYPE)

    # API selon la version de protobuf :
    #   - protobuf >= 5.x : fonction au niveau module message_factory.GetMessageClass()
    #   - protobuf 4.x    : méthode d'instance factory.GetMessageClass()
    #   - protobuf < 4.x  : méthode d'instance factory.GetPrototype()
    if hasattr(message_factory, "GetMessageClass"):
        message_class = message_factory.GetMessageClass(msg_desc)
    else:
        factory = message_factory.MessageFactory(pool=pool)
        if hasattr(factory, "GetMessageClass"):
            message_class = factory.GetMessageClass(msg_desc)
        else:
            message_class = factory.GetPrototype(msg_desc)

    return message_class, pool


def main():
    print("Préparation du benchmark...\n")

    # 1. Charger le descriptor (protoruf)
    descriptor_bytes = get_descriptor()

    # 2. Données de test
    original_obj = {
        "id": "123",
        "content": "Hello World! This is a benchmark.",
        "priority": 5,
        "tags": ["test", "example", "bench"],
        "metadata": {
            "author": "Alice",
            "created_at": 1234567890,
            "attributes": {"env": "prod", "version": "1.0"},
        },
    }
    json_str = json.dumps(original_obj)

    # Préparer le bytes protobuf pour le test de lecture
    proto_bytes = json_to_protobuf(json_str, descriptor_bytes, MESSAGE_TYPE)

    # 3. Configuration google.protobuf (si disponible)
    google_ok = HAS_GOOGLE_PROTOBUF
    if google_ok:
        try:
            MessageFactory, pool = create_google_message_factory(descriptor_bytes)

            # Créer un message google.protobuf à partir du JSON
            if callable(MessageFactory):
                # Si c'est une fonction, l'appeler
                google_msg = json_format.Parse(json_str, MessageFactory())
            else:
                # Si c'est une classe, l'instancier
                google_msg = json_format.Parse(json_str, MessageFactory())

            google_proto_bytes = google_msg.SerializeToString()

            print(f"✅ google.protobuf préparé avec succès")
            print(f"   Type de message : {type(google_msg).__name__}")
            print(f"   Taille binaire : {len(google_proto_bytes)} bytes\n")

        except Exception as e:
            print(f"⚠️  Erreur lors de la préparation google.protobuf : {e}")
            print("   Le benchmark ne portera que sur protoruf.\n")
            google_ok = False

    # 4. Lancement des benchmarks
    results = {}

    # ----- Écriture (JSON → Protobuf) -----
    # protoruf
    print(f"Benchmark écriture protoruf ({ITERATIONS:,} itérations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = json_to_protobuf(json_str, descriptor_bytes, MESSAGE_TYPE)
    t_protoruf_write = time.perf_counter() - start
    results["protoruf_write"] = t_protoruf_write
    print(f"  → {t_protoruf_write:.4f}s ({ITERATIONS / t_protoruf_write:,.0f} msg/s)\n")

    # google.protobuf
    if google_ok:
        print(f"Benchmark écriture google.protobuf ({ITERATIONS:,} itérations)...")
        start = time.perf_counter()
        if callable(MessageFactory):
            for _ in range(ITERATIONS):
                msg = json_format.Parse(json_str, MessageFactory())
                _ = msg.SerializeToString()
        else:
            for _ in range(ITERATIONS):
                msg = json_format.Parse(json_str, MessageFactory())
                _ = msg.SerializeToString()
        t_google_write = time.perf_counter() - start
        results["google_write"] = t_google_write
        print(f"  → {t_google_write:.4f}s ({ITERATIONS / t_google_write:,.0f} msg/s)\n")

    # ----- Lecture (Protobuf → JSON) -----
    # protoruf
    print(f"Benchmark lecture protoruf ({ITERATIONS:,} itérations)...")
    start = time.perf_counter()
    for _ in range(ITERATIONS):
        _ = protobuf_to_json(proto_bytes, descriptor_bytes, message_type=MESSAGE_TYPE)
    t_protoruf_read = time.perf_counter() - start
    results["protoruf_read"] = t_protoruf_read
    print(f"  → {t_protoruf_read:.4f}s ({ITERATIONS / t_protoruf_read:,.0f} msg/s)\n")

    # google.protobuf
    if google_ok:
        print(f"Benchmark lecture google.protobuf ({ITERATIONS:,} itérations)...")
        start = time.perf_counter()
        if callable(MessageFactory):
            for _ in range(ITERATIONS):
                msg = MessageFactory()
                msg.ParseFromString(google_proto_bytes)
                _ = json_format.MessageToJson(msg)
        else:
            for _ in range(ITERATIONS):
                msg = MessageFactory()
                msg.ParseFromString(google_proto_bytes)
                _ = json_format.MessageToJson(msg)
        t_google_read = time.perf_counter() - start
        results["google_read"] = t_google_read
        print(f"  → {t_google_read:.4f}s ({ITERATIONS / t_google_read:,.0f} msg/s)\n")

    # 5. Affichage du tableau comparatif
    print("=" * 80)
    print(f"Résultats pour {ITERATIONS:,} messages")
    print("=" * 80)
    header = f"{'Opération':<30} {'google.protobuf':<20} {'protoruf (Rust)':<20} {'Gain':<10}"
    print(header)
    print("-" * len(header))

    if google_ok:
        # Écriture (sérialisation)
        g_write = results["google_write"]
        p_write = results["protoruf_write"]
        gain_write = (1 - p_write / g_write) * 100
        print(
            f"{'Sérialisation (JSON→Proto)':<30} {g_write:>8.4f}s        {p_write:>8.4f}s        {gain_write:>5.1f}%"
        )

        # Lecture (parsing)
        g_read = results["google_read"]
        p_read = results["protoruf_read"]
        gain_read = (1 - p_read / g_read) * 100
        print(
            f"{'Parsing (Proto→JSON)':<30} {g_read:>8.4f}s        {p_read:>8.4f}s        {gain_read:>5.1f}%"
        )
    else:
        # Affichage seul pour protoruf
        print(
            f"{'Sérialisation (JSON→Proto)':<30} {'N/A':<20} {results['protoruf_write']:>8.4f}s        {'':<10}"
        )
        print(
            f"{'Parsing (Proto→JSON)':<30} {'N/A':<20} {results['protoruf_read']:>8.4f}s        {'':<10}"
        )

    # 6. Tableau projeté pour 1M messages
    print(f"\n📊 Projection pour 1,000,000 messages :\n")
    factor = 1_000_000 / ITERATIONS

    print(f"{'Opération':<30} {'google.protobuf':<15} {'protoruf (Rust)':<15}")
    print("-" * 60)

    if google_ok:
        print(
            f"{'Sérialisation (1M)':<30} {results['google_write'] * factor:>6.2f}s         {results['protoruf_write'] * factor:>6.2f}s"
        )
        print(
            f"{'Parsing (1M)':<30} {results['google_read'] * factor:>6.2f}s         {results['protoruf_read'] * factor:>6.2f}s"
        )

        # Accélération
        speedup_write = results["google_write"] / results["protoruf_write"]
        speedup_read = results["google_read"] / results["protoruf_read"]
        print(f"\n   → protoruf est {speedup_write:.1f}x plus rapide en écriture")
        print(f"   → protoruf est {speedup_read:.1f}x plus rapide en lecture")
    else:
        print(
            f"{'Sérialisation (1M)':<30} {'N/A':<15} {results['protoruf_write'] * factor:>6.2f}s"
        )
        print(
            f"{'Parsing (1M)':<30} {'N/A':<15} {results['protoruf_read'] * factor:>6.2f}s"
        )


if __name__ == "__main__":
    main()
