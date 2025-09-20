# Delete the tag locally
git tag -d v0.2.3

# Delete the tag on GitHub/remote
git push origin :refs/tags/v0.2.3

# Run cargo update to fix Cargo.lock
cargo update

# Commit the updated Cargo.lock
git add Cargo.lock
git commit -m "Update dependencies"

# Recreate the tag
git tag -a v0.2.3 -m "Release v0.2.3"

# Push everything
git push origin main
git push origin v0.2.3