# Contributor Setup

Photon is split into two repositories:

- [`PhotonBrowser/photon`](https://github.com/PhotonBrowser/photon) — the browser application, UI, Rust application logic, build tooling, and documentation.
- [`PhotonBrowser/photon-engine`](https://github.com/PhotonBrowser/photon-engine) — Photon Engine, based on Ladybird and modified for Photon.

The browser repository pins an exact Photon Engine commit through the `Engine/` Git submodule.

## Clone Photon

### SSH

```bash
git clone --recurse-submodules git@github.com:PhotonBrowser/photon.git
cd photon
```

### HTTPS

```bash
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
```

If you already cloned Photon without its submodule:

```bash
git submodule update --init --recursive
```

After cloning, the repository should look roughly like:

```text
photon/
├── Engine/        # Photon Engine submodule
├── README.md
└── ...
```

## Configure the Ladybird upstream remote

`Engine/` points to `PhotonBrowser/photon-engine`. Git submodules do not automatically carry extra remotes, so contributors working on the engine should add Ladybird as `upstream`.

```bash
cd Engine
git remote add upstream https://github.com/LadybirdBrowser/ladybird.git
git remote -v
```

You should have:

```text
origin    https://github.com/PhotonBrowser/photon-engine.git
upstream  https://github.com/LadybirdBrowser/ladybird.git
```

If `upstream` already exists, do not add it again.

Return to the browser repository with:

```bash
cd ..
```

## Browser-only changes

For changes to Photon itself, such as UI, Rust application logic, configuration, documentation, or build tooling, create a branch in the main repository:

```bash
git switch -c feature/my-change
```

Make your changes, then commit them normally:

```bash
git add .
git commit -m "Describe the change"
```

The `Engine/` submodule should remain on the commit already pinned by Photon unless your work also requires an engine change.

## Engine changes

`Engine/` is a Git repository of its own. A fresh submodule checkout is normally in detached-HEAD state, so create a real branch before editing it.

```bash
cd Engine
git switch master
git pull origin master
git switch -c feature/my-engine-change
```

Make and commit the engine changes inside `Engine/`:

```bash
git add .
git commit -m "Describe the engine change"
```

If you have push access to `PhotonBrowser/photon-engine`:

```bash
git push -u origin feature/my-engine-change
```

Otherwise, fork `PhotonBrowser/photon-engine` on GitHub, add your fork as a remote, and push the branch there:

```bash
git remote add fork git@github.com:YOUR_USERNAME/photon-engine.git
git push -u fork feature/my-engine-change
```

Open a pull request against `PhotonBrowser/photon-engine`.

## Updating Photon after an engine change

After the required engine commit has been merged into `PhotonBrowser/photon-engine`, update the engine revision pinned by Photon.

From the browser repository:

```bash
cd Engine
git switch master
git pull origin master
cd ..
```

Git will now show the submodule pointer as modified:

```bash
git status
```

Commit that pointer in the Photon repository:

```bash
git add Engine
git commit -m "Update Photon Engine"
```

The browser repository stores the exact engine commit it expects, so do not leave `Engine/` pointing at an unpushed local commit.

## Syncing Photon Engine with Ladybird

Photon Engine keeps Ladybird as an upstream remote.

To inspect new Ladybird changes:

```bash
cd Engine
git switch master
git fetch upstream
git log --oneline master..upstream/master
```

Maintainers can merge the latest Ladybird upstream into Photon Engine with:

```bash
git merge upstream/master
```

Resolve conflicts carefully, test the engine, and then push the resulting Photon Engine commit.

After the engine sync is pushed, update the `Engine/` submodule pointer in the main Photon repository as described above.

Do not merge Ladybird upstream directly into `PhotonBrowser/photon`.

## Updating an existing Photon checkout

Update the browser:

```bash
git pull
```

Then update the pinned engine checkout:

```bash
git submodule update --init --recursive
```

If the Photon commit changed the engine revision, this checks out the exact Photon Engine commit required by that browser revision.

## Repository rules

Keep these boundaries clear:

```text
PhotonBrowser/photon
├── browser UI
├── Rust application logic
├── tabs, sessions and configuration
├── build tooling
└── documentation

PhotonBrowser/photon-engine
├── Ladybird-derived web engine
├── rendering and web-platform changes
├── engine performance work
└── Photon-specific engine/embedding APIs
```

Do not place ordinary browser UI or application state inside Photon Engine.

Do not reach directly from UI code into arbitrary Ladybird internals. Photon-specific engine capabilities should go through a clean engine/embedding boundary.

## Quick setup check

From the root of the Photon repository:

```bash
git status
git submodule status
```

A clean checkout should show no unexpected modifications, and `Engine/` should be checked out at the commit pinned by Photon.

For engine work:

```bash
cd Engine
git remote -v
```

Confirm that `origin` points to `PhotonBrowser/photon-engine` and `upstream` points to `LadybirdBrowser/ladybird`.
