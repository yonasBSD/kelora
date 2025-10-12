# Releasing Kelora

Use the `just release-prepare` helper to keep version tagging and pre-release checks consistent.

1. Update `Cargo.toml` (and any docs) with the new version.
2. Commit the changes so the working tree is clean.
3. Run:

```bash
just release-prepare
```

The command will:

- read the version from `Cargo.toml` and make sure it is valid SemVer;
- ensure the version differs from the most recent `v*` git tag (helpful when you forget to bump the version);
- run `just docs-build` and `just check`;
- ensure the git tag `v<version>` does not already exist;
- create the new tag and print the push commands so you can run them manually.

Push the release when you are satisfied:

```bash
git push origin main
git push origin v<version>
```

You can also override the remote or branch shown in the instructions:

```bash
RELEASE_REMOTE=upstream RELEASE_BRANCH=release just release-prepare
```

If something goes wrong before pushing, delete the tag locally with `git tag -d v<version>`, fix the issue, and rerun `just release-prepare`.
