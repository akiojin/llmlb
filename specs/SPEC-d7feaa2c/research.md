# Research (deprecated)

This file previously documented the plugin-based engine architecture.
That approach has been removed in favor of in-process managers.

Current design summary:
- TextManager owns an EngineRegistry of built-in engines.
- AudioManager and ImageManager are thin wrappers for whisper.cpp and stable-diffusion.cpp.

For the current design, see:
- spec.md
- plan.md
- data-model.md
