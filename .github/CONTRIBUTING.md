# Contributing

Thanks for taking a look. This file says how the project is built, what the code
is expected to look like, and what happens to a pull request.

## Building

Windows 10 or 11. The app is Windows-only by design — it talks to the shell, the
registry and AppContainer directly.

You need:

- [Bun](https://bun.sh) — the package manager the project uses;
- [Rust](https://rustup.rs) — stable toolchain, MSVC target;
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) — Visual Studio
  Build Tools and WebView2, which recent Windows already ships.

```bash
bun install
bun run tauri dev      # development
bun run tauri build    # release build
bun run check          # type check (svelte-check)
```

The first Rust build takes minutes and fills `src-tauri/target` with tens of
gigabytes. That folder is ignored — keep it that way.

## What the code is expected to look like

The project has a voice, and it is worth matching.

**Comments explain why, not what.** A comment saying `// increment the counter`
above `count += 1` is noise. A comment saying which bug the line prevents, or
which two approaches were weighed and why this one won, is worth its space.
Comments in this project are written in Russian, in whole sentences.

**Every strange decision carries its reason.** If something looks wrong at first
glance and is nonetheless correct, say so on the spot. The next person to read it
will be you in six months.

**No user-visible text in the markup.** Everything goes through
`src/lib/data/language.ts`, in all three languages, and the key is requested as
`i18n.t.some_key`. A missing key fails the type check rather than showing an
empty space on screen.

**CSS is edited rule by rule.** Never by cutting ranges of lines out of a file —
that is how live rules get lost. Colours come from the tokens in
`src/lib/App/style/main/tokens.css`, and both themes are checked before a change
is called done.

**Svelte 5 runes only.** No stores, no `export let`. Transitions are local by
default — if an animation refuses to play, that is usually why.

## Pull requests

- Branch off `main`, one topic per pull request.
- `bun run check` must pass, and so must `cargo check` inside `src-tauri`.
- Say what you changed and why. A screenshot for anything visual, both themes.
- If it touches Windows settings, the registry or file permissions, say exactly
  which keys and paths — that gets read carefully.

## Reporting a bug

Use the [issue templates](https://github.com/Good4Fox/QuickRapidX/issues/new/choose).
The app version and the Windows build matter more than anything else: half of
what this program does depends on the exact build it is running on.

Security issues do not go into issues — see [SECURITY.md](../SECURITY.md).
