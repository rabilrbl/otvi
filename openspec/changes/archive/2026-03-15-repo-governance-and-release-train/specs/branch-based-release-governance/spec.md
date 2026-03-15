## ADDED Requirements

### Requirement: The repository MUST define branch responsibilities for contribution and release flow
The repository SHALL define `dev` as the default integration branch for normal pull requests and `main` as the branch that carries the next release train. Contributor guidance and pull request guidance MUST instruct contributors to target `dev` unless they are performing explicitly documented release-train work.

#### Scenario: Contributor prepares a normal change
- **WHEN** a contributor reads the repository contribution guidance for a feature, fix, or refactor
- **THEN** the guidance tells them to open the pull request against `dev`

#### Scenario: Maintainer prepares the next release train
- **WHEN** a maintainer reads the documented branch model
- **THEN** the documentation distinguishes `main` as the branch used to stage or stabilize the next release rather than the default feature-integration branch

### Requirement: Continuous integration MUST validate the defined release-train branches
The repository SHALL run continuous integration for pull requests targeting `dev` and for direct pushes to both `dev` and `main` so the integration branch and the release-train branch are both continuously validated.

#### Scenario: Pull request targets the integration branch
- **WHEN** a pull request targets `dev`
- **THEN** the configured CI workflow runs the repository's required validation jobs for that pull request

#### Scenario: Release-train branch receives changes
- **WHEN** commits are pushed to `main`
- **THEN** the configured CI workflow runs the repository's required validation jobs for the release-train branch state

### Requirement: Repository workflow documentation MUST cover GitHub settings needed to support the branch model
The repository SHALL document the GitHub settings required to make `dev` the effective default review target, including any administrator actions needed for default branch selection, default pull request base behavior, and branch protection.

#### Scenario: Maintainer enables the branch model in GitHub
- **WHEN** an administrator follows the repository's governance setup instructions
- **THEN** they can identify which GitHub settings must be updated outside the repository so the documented branch model works as intended
