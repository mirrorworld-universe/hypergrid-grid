# master branch
All new development occurs on the `master` branch.

Bug fixes that affect a `vX.Y` branch are first made on `master`.  This is to
allow a fix some soak time on `master` before it is applied to one or more
stabilization branches.

Merging to `master` first also helps ensure that fixes applied to one release
are present for future releases.  (Sometimes the joy of landing a critical
release blocker in a branch causes you to forget to propagate back to
`master`!)"

# Channels
Channels are used by end-users (humans and bots) to consume the branches
described in the previous section, so they may automatically update to the most
recent version matching their desired stability.

There are three release channels that map to branches as follows:
* edge - tracks the `master` branch, least stable.
* beta - tracks the largest (and latest) `vX.Y` stabilization branch, more stable.
* stable - tracks the second largest `vX.Y` stabilization branch, most stable.

## Steps to Create a Branch

### Create the new branch
1. Check out the latest commit on `master` branch:
    ```
    git fetch --all
    git checkout upstream/master
    ```
1. Determine the new branch name.  The name should be "v" + the first 2 version fields
   from Cargo.toml.  For example, a Cargo.toml with version = "0.9.0" implies
   the next branch name is "v0.9".
1. Create the new branch and push this branch to the `solana` repository:
    ```
    git checkout -b <branchname>
    git push -u origin <branchname>
    ```

Alternatively use the Github UI.

### Update master branch to the next release minor version

1. After the new branch has been created and pushed, update the Cargo.toml files on **master** to the next semantic version (e.g. 0.9.0 -> 0.10.0) with:
     ```
     $ scripts/increment-cargo-version.sh minor
     ```
1. Push all the changed Cargo.toml and Cargo.lock files to the `master` branch with something like:
    ```
    git co -b version_update
    git ls-files -m | xargs git add
    git commit -m 'Bump version to X.Y+1.0'
    git push -u origin version_update
    ```
## Steps to Create a Release

### Create the Release Tag on GitHub

1. Click "Draft new release".  The release tag must exactly match the `version`
   field in `/Cargo.toml` prefixed by `v`.
   1.  If the Cargo.toml version field is **0.12.3**, then the release tag must be **v0.12.3**
1. Make sure the Target Branch field matches the branch you want to make a release on.
   1.  If you want to release v0.12.0, the target branch must be v0.12
   1. If this is a patch release, review all the commits since the previous release on this branch and add details as needed.
1. Click "Save Draft", then confirm the release notes look good and the tag name and branch are correct.
1. Ensure all desired commits (usually backports) are landed on the branch by now.
1. Ensure the release is marked **"This is a pre-release"**.  This flag will need to be removed manually after confirming the Linux binary artifacts appear at a later step.
1. Go back into edit the release and click "Publish release" while being marked as a pre-release.
1. Confirm there is new git tag with intended version number at the intended revision after running `git fetch` locally.
