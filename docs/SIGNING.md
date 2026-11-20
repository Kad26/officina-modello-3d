# Signing the releases

## Where it stands

| Platform | Today | What people see |
| --- | --- | --- |
| macOS | ad-hoc signed (whole app, hardened runtime); not notarized | "Apple could not verify SketchCraft": **System Settings › Privacy & Security › Open Anyway** |
| Windows | unsigned | SmartScreen: **More info › Run anyway** |
| Linux | no signing needed | nothing |

## macOS: Developer ID and notarization

The release workflow signs and notarizes automatically once these
secrets exist in the `release` Environment and the `APPLE_SIGNING` variable is `true`.
Without them it builds exactly as today.

1. Join the Apple Developer Program (99 USD a year) at developer.apple.com.
2. In Xcode or the developer site, create a **Developer ID Application**
   certificate and export it from Keychain Access as a `.p12` with a password.
3. Add the secrets to the **`release` Environment** (Settings › Environments › release › Environment secrets; not
   repository secrets: only a job that names the environment, after its reviewer approves, can read them):

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | the `.p12`, base64: `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12`'s password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` (`security find-identity -v -p codesigning`) |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | an **app-specific password** from appleid.apple.com (not your Apple ID password) |
| `APPLE_TEAM_ID` | the 10-character team id |

4. Switch it on with a repository **variable** (not a secret): Settings › Secrets and variables › Actions › Variables
   › `APPLE_SIGNING` = `true`. Without it the workflow builds unsigned, as today.
5. Push a new tag. The `release` reviewer is asked twice (`sign-macos`, then `publish`). The DMG and app are then signed with your Developer ID
   and notarized, and open with no warning.

## App updates

The desktop app updates itself from GitHub Releases: it reads `latest.json`
from the newest release, downloads the update for its platform, and installs
it only if it was signed with SketchCraft's update key. The public half is in
`app/src-tauri/tauri.conf.json`; the private half is
`~/.tauri/sketchcraft-updater.key` on the maintainer's Mac (no password; see below).

The release workflow needs that private key as a repository secret, or the
build fails:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY -R jasonsutter87/sketchcraft < ~/.tauri/sketchcraft-updater.key
```

**Back the key up** (a password manager is fine). If it's lost, installed
copies can't verify anything newer, and everyone has to download one new
version by hand. Never commit it.

Every installer type updates itself (macOS app, Windows `.msi` and setup
`.exe`, Linux `.AppImage`, `.deb` and `.rpm`; Linux packages ask for an admin
password). If an install fails, the card offers the download page instead.

## Checking a download

Every release lists `SHA256SUMS.txt` with the SHA-256 of each file on it. After
downloading: `shasum -a 256 SketchCraft_*.dmg` (macOS/Linux) or
`Get-FileHash SketchCraft_*_x64-setup.exe` (Windows) and compare. Only download
from this repository's Releases page.

## Protecting the release

What the workflow does for you: releases build only from a `v*` tag on a commit
that's on `main` and matches the app's version, after CI passes; actions are
pinned to commit SHAs; npm install scripts don't run; and compiling is kept apart from
signing. The `build` job compiles with `--locked`, holds no update key and only a
read-only token (so no dependency's build script or proc-macro can read the key);
it signs its update packages with a throwaway key made for that run. The `publish`
job, which compiles nothing, signs them for real with the key from the `release`
environment and creates the draft release. The Apple secrets (once added, as `release` Environment secrets) go only to
steps of the `sign-macos` job, which names that environment, compiles nothing and checks out no code: it
takes the unsigned macOS app the `build` job made (built with no secrets, like every
platform), signs and notarizes it with Apple's own tools, and hands it to `publish`.
That job has never run, so watch the first tag after adding the Apple secrets (it
rebuilds the disk image with `hdiutil`, so the dmg's window layout is plain).

What only the repository owner can switch on (GitHub settings):

1. **Settings › Environments › New environment `release`**: add yourself under
   *Required reviewers* and move `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` from repository secrets into that
   environment. The `publish` job is the only one that names it, so a release
   waits for your approval before the key is available. The `APPLE_*` secrets go
   into the same environment, and `sign-macos` names it too (see "macOS: Developer ID and notarization" above).
   Until you do this, a tag push signs and drafts a release unattended.
2. **Settings › Rules › Rulesets › New tag ruleset**: target `v*`, restrict
   creation and deletion to yourself (no bypass), so a collaborator's tag can't
   start a release.
3. Turn on two-factor authentication (a passkey) for the account, and
   *Settings › General › Releases › Enable release immutability* so published
   releases can't be edited.

The updater key: it has no password today, so anything that can read the file
can sign an update. Adding a password means generating a **new** key, and the
public half is built into every installed copy, so a new key can't update
anyone already installed (they would download one new version by hand). Only do
that deliberately, if ever; keep the existing key out of backups you don't
control and off shared machines. The app already refuses updates that aren't
newer than the installed version or whose download isn't under that version's own
release address.

## Windows

Options, cheapest first:

- **SignPath Foundation**: free code signing for open-source projects
  (apply at signpath.org; they sign builds from CI).
- **Azure Trusted Signing**: about 10 USD a month; needs identity
  validation. Tauri can call it through `bundle.windows.signCommand`.
- An OV or EV certificate from a certificate authority.

Each needs its own small workflow change; tell Claude which you pick.

The release workflow could not be run while it was changed (it needs GitHub). On the
first tag, watch the `build` and `publish` jobs: `publish` expects one `.app.tar.gz`,
one `-setup.exe` and one `.AppImage` among the built files, signs each with
`tauri signer sign` and writes `latest.json` itself. If a step fails, the draft
isn't created and nothing is published.
