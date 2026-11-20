# Publishing a release

The Release workflow builds Windows x64 ZIP, Linux x64 Debian/AppImage, and
macOS Apple Silicon ZIP packages, then attaches them to a GitHub Release.

Enable GitHub Actions in the fork's Actions tab if GitHub has disabled inherited
workflows. Tag-triggered releases use the workflow from the tagged commit.

Push a version tag on the commit to publish:

```sh
git tag v0.2.2-plus.1
git push origin v0.2.2-plus.1
```

Alternatively, after this workflow is on the default branch, open
**Actions → Release → Run workflow**, select the branch to build, and enter a
new version tag such as `v0.2.2-plus.1`. The workflow creates the tag and Release
after all three platform builds succeed. Existing tags pointing to a different
commit are rejected.

Use a new version tag for each release. Workflow and build failures appear in
the Actions run; packages are published only after all platforms finish.
No game ROM, save, snapshot, or device key is included.
