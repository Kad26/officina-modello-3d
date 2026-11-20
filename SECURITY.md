# Security

SketchCraft opens files from anywhere and runs plugins and AI agents, so we take
security seriously: every release goes through repeated attack-and-fix rounds
before it ships.

## Reporting a problem

Please **don't open a public issue** for a security problem. Instead use
GitHub's private report form:
[**Report a vulnerability**](https://github.com/jasonsutter87/sketchcraft/security/advisories/new).

Include what you found, how to reproduce it (a sample file or steps), and what
an attacker could do with it. We'll reply within a few days, fix it in a
release, and credit you in the advisory if you'd like.

## What counts

- A file (.skp, .dxf, .obj, .sketchcraft…) that crashes SketchCraft, hangs it,
  uses huge amounts of memory, or reads or writes files it shouldn't
- A plugin or SketchUp Ruby script that escapes its sandbox
- An AI agent (MCP) or the local bridge doing something the person didn't allow
- Anything in the updater or the release downloads

## Supported versions

Fixes go into the latest release. The app updates itself, so please stay current.
