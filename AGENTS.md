# Working agreements

- Discuss new requirements, implementation boundaries, dependencies, and risks before implementing. An accepted implementation task authorizes the necessary code and tests.
- Keep the CLI and Skills small. Use mature libraries for HTTP, CLI parsing, and OS credentials; do not build an agent framework or query engine here.
- This repository is public. Never commit credentials, real user logs, local experiment files, or private deployment details.
- The CLI uses only public user APIs and has no dependency on the private core repository.
- Distinguish planned, implemented, locally verified, platform verified, and released capabilities in documentation.
- Preserve unrelated worktree changes. Do not publish, push, or create a release unless requested.
