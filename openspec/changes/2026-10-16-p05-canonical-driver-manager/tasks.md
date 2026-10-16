# Tasks

- [x] 1. Add a failing external consumer test proving both public DriverManager paths share one registry, callback identity, login timeout, and diagnostic writer.
- [x] 2. Move the unique manager into `druid-rdbc`, leave the Druid path as an exact re-export, and pass focused regressions plus strict formatting and Clippy.
- [x] 3. Correct the approved `java.sql.DriverManager` canonical mapping, freeze its new digest without changing OpenJDK source records, and prove the LIVE manager capability with the external regression, semantic phase, and public API gates.
- [ ] 4. Run the full no-physical-standard and SemVer gates, record CI results, then sync and archive the change.
- [ ] 5. Commit and push each completed feature point.
