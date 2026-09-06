# ADR 0009 — `patch-package` (new devDependency) + forcing C++17 for the `fmt` CocoaPod

**Status:** accepted; audio patch refreshed for 0.11.7 on 2026-08-28. **Date:** 2026-08-05.

## Context

Getting a real iOS build past this host's now-updated Xcode 26.6 toolchain (see
`docs/REVIEW-2026-08-04-L8.md` §10 for the CoreSimulator fix that unblocked attempting an iOS
build at all) surfaced two real, pre-existing native-toolchain incompatibilities — neither touched
by any prior session work:

1. `fmt` 11.0.2 (a transitive React Native CocoaPod) fails to compile under Xcode 26.6's Clang:
   every `FMT_COMPILE_STRING` call site errors with "call to consteval function ... is not a
   constant expression." `fmt`'s own header (`base.h`) decides `FMT_USE_CONSTEVAL`/`FMT_CONSTEVAL`
   via an unconditional `#define` chain with no `#ifndef` guard, so passing `-DFMT_USE_CONSTEVAL=0`
   on the compiler command line is silently overwritten by the header itself and does nothing.
2. `react-native-audio-api@0.9.3`'s `Constants.h` uses `size_t` without including `<cstddef>` —
   works under most toolchains because `<cmath>`/`<limits>` often transitively provide it, but not
   under this Xcode/libc++ combination.

## Decision

- **Podfile** (`ios/Podfile`, `post_install`): force `CLANG_CXX_LANGUAGE_STANDARD = 'c++17'` for
  just the `fmt` target. One of `fmt`'s own `FMT_USE_CONSTEVAL` conditions is
  `FMT_CPLUSPLUS < 201709L` (pre-C++20) → 0 — this makes fmt's own logic disable consteval
  honestly, instead of fighting an unconditional macro redefinition that always loses. `fmt`
  supports C++11+, so downgrading only this one pod's standard is safe.
- **`patch-package`** (new devDependency, C3): the `size_t` fix has to live in
  `node_modules/react-native-audio-api`, which `npm install` would otherwise wipe on every fresh
  install/CI run. `patch-package` is the standard, minimal way to persist a one-line upstream fix
  without forking the whole package — devDependency only, not shipped in the app bundle, wired via
  a `postinstall` script so it applies automatically. Patch file:
  `patches/react-native-audio-api+0.11.7.patch` (adds `#include <cstddef>` to `Constants.h`). The
  0.9.3 patch was deleted during the 0.11 upgrade; it had accidentally captured about 40,000 lines
  of local Android build outputs in addition to the one intended source line.

## Consequences

- Both fixes are host-toolchain-version-specific workarounds, not app logic changes — 0.11.7 still
  uses unqualified `size_t` without `<cstddef>`, so the one-line patch remains necessary. Revisit if
  `fmt` or `react-native-audio-api` ship upstream fixes for these (check on the next version bump
  of either; delete the patch/Podfile hook if the underlying bug is gone).
- `npm run setup` (`scripts/setup.sh`) already runs `npm install`, so the patch applies
  automatically on a fresh machine — no extra manual step needed after this ADR lands.
