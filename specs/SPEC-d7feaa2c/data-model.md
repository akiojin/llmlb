# Data Model: Manager-based engines

## ModelDescriptor (from model manifest)
ModelDescriptor is provided by the model registry/manifests and includes:
- model_id
- model_dir
- format (gguf, safetensors, ...)
- runtime (llama_cpp, safetensors_cpp, ...)
- architectures (optional)
- metadata (optional; may include benchmark hints)

## EngineRegistration
```
struct EngineRegistration {
    std::string engine_id;
    std::string engine_version;
    std::vector<std::string> formats;
    std::vector<std::string> architectures;
    std::vector<std::string> capabilities; // text, embeddings, ...
};
```

## EngineRegistry
EngineRegistry is an in-process registry owned by TextManager.
It maps runtime -> list of engines and resolves by format/capability/architecture.

```
class EngineRegistry {
public:
    bool registerEngine(EngineHandle engine, const EngineRegistration& registration, std::string* error);
    Engine* resolve(const ModelDescriptor& descriptor, const std::string& capability) const;
    std::vector<std::string> getRegisteredRuntimes() const;
};
```

## TextManager
TextManager creates the EngineRegistry and registers built-in engines.

```
class TextManager {
public:
    Engine* resolve(const ModelDescriptor& descriptor, const std::string& capability, std::string* error);
    std::vector<std::string> getRegisteredRuntimes() const;
private:
    std::unique_ptr<EngineRegistry> registry_;
};
```

## AudioManager / ImageManager
AudioManager wraps WhisperManager. ImageManager wraps SDManager.
They are static, in-process managers (no plugin loading).

## Flow (simplified)

```
ModelStorage -> ModelDescriptor
                 |
                 v
InferenceEngine -> TextManager -> EngineRegistry -> Built-in Engine
```
