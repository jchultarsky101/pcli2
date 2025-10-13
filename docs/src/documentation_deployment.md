# Documentation Deployment Instructions

## GitHub Pages Setup

To deploy the Oranda-generated documentation to GitHub Pages, follow these steps:

1. **Enable GitHub Pages**:
   - Go to your repository settings: https://github.com/jchultarsky101/pcli2/settings
   - Scroll down to the "Pages" section
   - Under "Source", select "GitHub Actions" as the source
   - Click "Save"

2. **Trigger the Documentation Deployment**:
   - Push a commit to the main branch to trigger the documentation workflow
   - Or manually trigger the workflow from the GitHub Actions page

3. **Access Your Documentation**:
   - Once the workflow completes successfully, your documentation will be available at:
     https://jchultarsky101.github.io/pcli2

## Local Development

To preview the documentation locally:

```bash
# Install Oranda if you haven't already
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/oranda/releases/latest/download/oranda-installer.sh | sh

# Build the documentation
oranda build

# Serve locally (if you have a simple HTTP server)
cd public
python3 -m http.server 8000
# Then visit http://localhost:8000
```

## Workflow Details

The documentation workflow (`documentation.yml`) will:
- Automatically build documentation on pushes to the main branch
- Deploy the documentation to GitHub Pages
- Run on manual triggers via workflow_dispatch

## Troubleshooting

If the documentation doesn't appear:
1. Check that GitHub Pages is set to use "GitHub Actions" as the source
2. Verify the documentation workflow ran successfully
3. Check the workflow logs for any errors