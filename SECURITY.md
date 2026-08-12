# Security Policy

## Supported versions

QuickRapidX is in active development, so only the latest release gets fixes.
There are no long-lived branches and no back-porting.

| Version | Supported |
| ------- | --------- |
| 0.5.x   | ✅ |
| < 0.5   | ❌ |

## What the app can do to a machine

Worth knowing before you report — and before you build from source. QuickRapidX
touches the system in ways an ordinary program does not:

- writes to `HKCU` — Recycle Bin limits, accessibility hotkey prompts, autostart;
- asks for administrator rights for component store repair, hibernation and the
  removal of `Windows.old`;
- takes ownership of files and rewrites their permissions when deleting
  `Windows.old`;
- launches other programs, including with cut-down rights through AppContainer,
  Windows Sandbox and Sandboxie;
- creates shortcuts and pins them to the taskbar and the Start menu.

Isolation as a whole is **experimental**. Treat it as a convenience, not as a
security boundary: it narrows what a program can reach, it does not contain a
program that actively tries to escape.

## Reporting a vulnerability

Report privately, not through a public issue.

- **GitHub Security Advisories** — the [Security tab](https://github.com/Good4Fox/QuickRapidX/security/advisories/new)
  of this repository. Preferred: the report stays private until a fix ships.

Please include the version, the Windows build, what you did, what happened, and
what you expected. A crash log or the exact registry path helps more than a
screenshot.

**What to expect.** A first reply within seven days. If the report is accepted,
you will hear when a fix is planned and when it ships, and you will be credited
in the changelog unless you ask otherwise. If it is declined, you will be told
why — silence is not an answer we give.

## Out of scope

- Anything requiring an attacker who already has administrator rights on the
  machine: at that point the game is already over, with or without this app.
- Escaping from isolation. See above — it is not a security boundary and does
  not claim to be.
- Vulnerabilities in Windows itself, in Sandboxie, or in programs launched
  through QuickRapidX. Report those to their authors.
