# Create the tag
git tag -a v0.2.3 -m "Release v0.2.3"
git push origin v0.2.3

Something went wrong?

# Delete the tag locally
git tag -d v0.2.3

# Delete the tag on GitHub/remote
git push origin :refs/tags/v0.2.3
