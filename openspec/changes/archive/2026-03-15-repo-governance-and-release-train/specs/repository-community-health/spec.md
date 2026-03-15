## ADDED Requirements

### Requirement: The repository MUST separate project orientation from contributor and maintainer guidance
The repository SHALL provide a concise root `README.md` for project overview, quick start, architecture summary, and navigation to deeper materials. Contributor, support, security, conduct, and release-process guidance MUST live in dedicated Markdown files that are linked from the README instead of being duplicated there.

#### Scenario: New contributor lands on the repository
- **WHEN** a user opens the root repository page on GitHub
- **THEN** `README.md` presents a short project introduction and links to dedicated contributor and maintainer guidance files rather than embedding all operational reference material inline

#### Scenario: Maintainer needs process-specific guidance
- **WHEN** a maintainer needs instructions for contribution flow, support routing, security reporting, or release preparation
- **THEN** the repository contains dedicated Markdown files for those topics with clear, purpose-specific guidance

### Requirement: The repository MUST provide structured GitHub intake templates
The repository SHALL include GitHub issue templates for at least bug reports, feature requests, and documentation issues, and SHALL include a pull request template that captures summary, validation performed, documentation impact, and release impact.

#### Scenario: User opens a new issue
- **WHEN** a user starts creating an issue in GitHub
- **THEN** GitHub presents repository-provided templates that request the information needed for the selected issue type

#### Scenario: Contributor opens a pull request
- **WHEN** a contributor opens a pull request
- **THEN** the repository presents a pull request template that asks for testing details, affected docs, and any release-train implications

### Requirement: Repository guidance MUST distinguish repository-managed policy from GitHub-admin settings
The repository SHALL document which contribution and governance expectations are enforced by tracked files and workflows, and which expectations require GitHub administrative configuration outside the repository.

#### Scenario: Administrator configures contribution settings
- **WHEN** a maintainer reads the repository governance documentation
- **THEN** the documentation explicitly lists settings such as default branch, branch protection, and default pull request target behavior that must be configured in GitHub
