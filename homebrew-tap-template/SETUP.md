# Homebrew Tap Setup Guide

This guide explains how to create and configure the `dloss/homebrew-kelora` tap repository.

## Step 1: Create the Tap Repository

1. Go to GitHub and create a new repository named: **`homebrew-kelora`**
   - Owner: `dloss`
   - Repository name: `homebrew-kelora` (must follow this naming convention)
   - Description: "Homebrew tap for Kelora"
   - Public repository (required for Homebrew taps)
   - Initialize with a README (optional, we'll replace it)

2. Clone the repository locally:
   ```bash
   git clone https://github.com/dloss/homebrew-kelora.git
   cd homebrew-kelora
   ```

## Step 2: Set Up Repository Structure

Copy the files from this template directory into the tap repository:

```bash
# From the kelora repo root
cp homebrew-tap-template/Formula/kelora.rb ~/path/to/homebrew-kelora/Formula/kelora.rb
cp homebrew-tap-template/README.md ~/path/to/homebrew-kelora/README.md
```

Your tap repository should have this structure:
```
homebrew-kelora/
├── Formula/
│   └── kelora.rb
└── README.md
```

## Step 3: Get SHA256 Checksums for Current Release

Download the current release binaries and calculate their SHA256 checksums:

```bash
# Download current release (v1.4.2)
curl -LO https://github.com/dloss/kelora/releases/download/v1.4.2/kelora-aarch64-apple-darwin.tar.gz
curl -LO https://github.com/dloss/kelora/releases/download/v1.4.2/kelora-x86_64-apple-darwin.tar.gz

# Calculate checksums
shasum -a 256 kelora-aarch64-apple-darwin.tar.gz
shasum -a 256 kelora-x86_64-apple-darwin.tar.gz
```

Update `Formula/kelora.rb`:
- Replace `PLACEHOLDER_ARM64_SHA256` with the arm64 checksum
- Replace `PLACEHOLDER_X86_64_SHA256` with the x86_64 checksum

## Step 4: Test the Formula Locally

Before pushing, test that the formula works:

```bash
# Tap your local repository
brew tap dloss/kelora /path/to/homebrew-kelora

# Install and test
brew install kelora
kelora --version

# Uninstall (if needed)
brew uninstall kelora
brew untap dloss/kelora
```

## Step 5: Push to GitHub

```bash
cd ~/path/to/homebrew-kelora
git add .
git commit -m "Initial Homebrew formula for Kelora v1.4.2"
git push origin main
```

## Step 6: Configure GitHub Personal Access Token (for Auto-Updates)

The Kelora release workflow will automatically update the tap formula when new versions are released. For this to work, you need to create a GitHub Personal Access Token (PAT).

1. Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Click "Generate new token (classic)"
3. Configure the token:
   - **Note**: "Kelora Homebrew Tap Auto-Update"
   - **Expiration**: 1 year (or "No expiration" if you prefer)
   - **Scopes**: Select:
     - `repo` (Full control of private repositories) - this gives access to public repos too
   - Alternatively, use fine-grained permissions:
     - Repository access: Only select repositories → `homebrew-kelora`
     - Permissions: Contents (Read and write), Pull requests (Read and write)

4. Click "Generate token" and **copy the token immediately** (you won't see it again)

5. Add the token as a secret in the **kelora repository** (not the tap repo):
   - Go to `https://github.com/dloss/kelora/settings/secrets/actions`
   - Click "New repository secret"
   - Name: `HOMEBREW_TAP_TOKEN`
   - Value: Paste the token
   - Click "Add secret"

## Step 7: Verify Auto-Update Works

The auto-update workflow will run on the next release. To test it:

1. Wait for the next Kelora release (or create a test release)
2. The release workflow will automatically:
   - Download the new macOS binaries
   - Calculate SHA256 checksums
   - Update the formula in the tap repository
   - Create a pull request (or direct commit)
3. Review and merge the PR (if using PR mode)

## User Installation

Once the tap is set up, users can install Kelora with:

```bash
brew tap dloss/kelora
brew install kelora
```

Or in one command:
```bash
brew install dloss/kelora/kelora
```

## Maintenance

### Manual Formula Updates

If you need to manually update the formula:

1. Edit `Formula/kelora.rb` in the tap repository
2. Update the version number
3. Update the download URLs
4. Update the SHA256 checksums (see Step 3)
5. Commit and push

### Formula Audit

Homebrew provides a linting tool:

```bash
brew audit --strict --online dloss/kelora/kelora
```

### Testing Formula Changes

```bash
brew reinstall dloss/kelora/kelora
brew test dloss/kelora/kelora
```

## Troubleshooting

### Users Can't Find the Tap

Make sure:
- Repository is named exactly `homebrew-kelora`
- Repository is public
- Formula is in `Formula/kelora.rb` (not `formula/` or `Formulas/`)

### SHA256 Mismatch Errors

If users get checksum errors:
- Verify the SHA256 checksums in the formula match the actual binaries
- Ensure you're downloading from the correct release tag
- Re-download and re-calculate checksums if needed

### Installation Fails

Check:
- Binary is executable after extraction
- Binary name matches what's being installed (`kelora`)
- Tarball structure is correct (binary should be at root)

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae)
- [How to Create and Maintain a Tap](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
