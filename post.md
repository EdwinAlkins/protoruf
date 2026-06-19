
Few days ago, I got officially tired of installing protoc in CI pipelines and regenerating _pb2.py files every time a schema changed. So I built protoruf — a Python library in Rust that compiles .proto files in memory, no external tools needed.

The goal was simple: to make using Protobuf in Python as smooth as working with Pydantic. And it worked:
• pip install protoruf — that's it
• compile_proto() compiles on the fly
• json_to_protobuf() / protobuf_to_json() in one line
• Direct Pydantic integration (pydantic_to_protobuf / protobuf_to_pydantic)

But I decided not to stop there. The default free functions re-decode the descriptor on every call, which costs a bit of speed. So I introduced DescriptorCache — a one-liner that caches the descriptor pool and makes repeated conversions blazing fast.

The result: with the cache, protoruf is now ~5× faster for writing and ~11× for reading than google.protobuf in high-throughput scenarios. And you still get zero generated files, no protoc, and Pydantic validation out of the box.

I wrote a full breakdown with before/after code on Dev.to, including some benchmarks for both the simple and cached paths.
If you've ever fought with protoc, you'll probably relate 😅

📦 pip install protoruf (v0.1.5)
🔗 Full read: [link to your Dev.to article]
💻 GitHub: https://github.com/EdwinAlkins/protoruf

#python #protobuf #developerexperience #pydantic #rust