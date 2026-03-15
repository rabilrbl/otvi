## MODIFIED Requirements

### Requirement: The documentation site MUST expose versioned docs using Docusaurus-supported versioning
The documentation site MUST use Docusaurus' built-in docs versioning so maintainers can preserve release snapshots alongside the in-progress documentation set. The site MUST present the latest released documentation version as the default public docs experience, while still allowing users to access the unreleased current docs through Docusaurus version navigation. Version metadata and generated versioned-doc artifacts MUST be stored in the repository using the standard Docusaurus structure.

#### Scenario: Site renders latest release as default and unreleased docs separately
- **WHEN** the docs site is built after at least one version snapshot has been created
- **THEN** users land on the latest released docs by default and can also select the current unreleased docs from the version navigation

#### Scenario: Version metadata is repository-backed
- **WHEN** a maintainer creates a new documentation version
- **THEN** the generated version metadata and versioned docs files are checked into the repository in the standard Docusaurus versioning locations rather than being created only at deploy time

### Requirement: Docs publishing MUST run automatically for release tags
The CI/CD workflow MUST build and deploy the documentation site automatically when a new server release tag matching the documented release pattern is pushed, using the repository state associated with that tag. The published site for that run MUST promote the tagged release documentation as the latest stable docs version rather than leaving the unreleased current docs as the default public landing view.

#### Scenario: Release tag triggers docs deploy
- **WHEN** a new server release tag that matches the configured release pattern is pushed to the repository
- **THEN** GitHub Actions runs the docs build and GitHub Pages deployment workflow without requiring a manual follow-up step

#### Scenario: Tagged deployment publishes the matching stable docs version
- **WHEN** the docs deployment workflow runs for a matching server release tag
- **THEN** the built site content comes from the checked-out tagged ref and the tagged docs version becomes the site's latest stable documentation selection
