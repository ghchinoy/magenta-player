# Monorepo Engineering Notes

Cross-cutting lessons learned building on top of MRT2. Each document focuses
on one domain and captures the non-obvious things — the problems that took
time to debug and would be expensive to rediscover.

| Document | Summary |
| :--- | :--- |
| [realtime-audio.md](realtime-audio.md) | Ring buffer sizing, priming, AVAudioSourceNode patterns |
| [cpp-swift-ffi.md](cpp-swift-ffi.md) | C bridge pattern, SPM + static C++ libraries, ObjC++ |
| [build-system.md](build-system.md) | cmake gotchas, libtool merging, Makefile absolute-path patterns |
| [mrt2-integration.md](mrt2-integration.md) | MRT2-specific: init_assets, resources layout, 25 Hz cadence |
