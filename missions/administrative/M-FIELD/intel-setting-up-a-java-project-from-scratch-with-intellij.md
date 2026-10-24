# Setting up a Java project from scratch with IntelliJ

Mission: M-FIELD. Status: notes in progress. Jim plans to relocate these notes later. The source is Jim's field test of tsk on 2026-10-24, in a new private repo that is a practice environment for writing Java by hand for a coding interview.

Each section has a question from Jim, the answer, and how sure the answer is. "Checked" means someone ran it. "From memory" means it comes from the author's knowledge and has not been run on Jim's machine.

## Order of setup

Question: can the ledger, `AGENTS.md` and a README come before the Java project, or after?

Answer: create the Java project first, then add the rest.

- The IntelliJ new-project wizard works best in an empty directory and warns when the directory has files. It generates the Gradle wrapper, `.idea` and a `.gitignore`.
- Leave "Create Git repository" unticked in the wizard when the directory is already a repo.
- tsk does not conflict with the project files. The ledger is outside the working tree. `tsk install-plugin` adds only `.claude/settings.json` and `.tsk/config.toml`.
- The first `tsk ledger push` or `tsk thread start` creates the ledger on `origin`.
- Claude Code reads `CLAUDE.md`, not `AGENTS.md`. To keep one file for several tools, write `AGENTS.md` and put the line `@AGENTS.md` in `CLAUDE.md`.

Sequence: 1) run the wizard with Gradle and no Git init, 2) check `.gitignore` covers `.idea`, `build` and `.gradle`, 3) commit and push, 4) add `README.md`, run `tsk install-plugin claude-cli`, commit what it writes, 5) add `AGENTS.md`.

How sure: from memory for the IntelliJ wizard behaviour; checked for the tsk side (0.26.0 source).

## Q1: use the JDK that mise installed

Question: Jim uses mise on his local machine. How does IntelliJ use the mise-installed JDK?

Answer, in the order to try:

1. Find the install path in a terminal inside the repo: `mise where java`. It prints a directory under `~/.local/share/mise/installs/java/<version>`. If nothing is installed for the repo yet, run `mise use java@<version>` first (for example `mise use java@temurin-21`). This writes `.mise.toml` in the repo, which records the version for the project.
2. Add that directory as an SDK in IntelliJ: File, Project Structure (Cmd+;), Platform Settings, SDKs, the plus sign, Add JDK from disk, and select the directory from step 1. It must be the directory that contains `bin/java`, not the `bin` directory.
3. Set the project SDK to it: Project Structure, Project Settings, Project, SDK.
4. Set the Gradle JVM to the same JDK: Settings, Build, Execution, Deployment, Build Tools, Gradle, Gradle JVM. If this is left at the default, Gradle can run on a different Java from the code.
5. Pin the version for Gradle in `build.gradle.kts` with a toolchain, so a terminal build and the IDE build use the same Java:

```kotlin
java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}
```

Points to know:

- The path contains the version number. After `mise upgrade` or a change of version the old path can disappear, and IntelliJ then shows the SDK in red. Add the new path and set it again.
- IntelliJ started from the Dock or Spotlight does not read your shell, so `mise` shims and `JAVA_HOME` set in `.zshrc` are not visible to it. Add the SDK by path as in step 2 and do not rely on `JAVA_HOME`.
- From a terminal in the repo, `java -version` and `./gradlew --version` should print the same version as the IDE. A mismatch means one of them is not using the mise JDK.

Not yet checked, and worth testing on Jim's machine: mise has a setting for macOS that links its JDKs into the place where macOS and IDEs look for JDKs, so IntelliJ lists them without adding by path. The setting name and its exact behaviour are from memory only. This session could not reach the mise documentation (`mise.jdx.dev` is not reachable from the session's network). Read the Java page of the mise documentation before relying on it.

How sure: steps 1 to 5 are from memory of IntelliJ and mise behaviour, not run. Jim's result replaces this note.

## Q2: which Java version

Question: which version should the project use, and what is the latest stable version?

Answer: Java 25, the current long-term support (LTS) release, released in September 2025. The next LTS is Java 29, due in September 2027. Java 26 was released in March 2026. Java 27 is also released: Jim's mise reported `java@27.0.1.0.2` on 2026-10-24. Neither is an LTS release, and each stops getting updates within about six months, so neither suits a long-lived practice repo.

Two exceptions:

- Match the interview. Ask which Java version the company uses, or read the job advert. Java 17 and 21 are common in production code, and the language practised should not be newer than the language used on the day. With no information, Java 21 is the safe middle: it has records, `switch` patterns and sealed types, and interview platforms accept it.
- Check the tools. Java 25 needs a recent Gradle (9.1 or later, from memory) and a recent IntelliJ. An older Gradle fails with an unsupported-version error. Check with `./gradlew --version`.

How sure: from memory. The author's knowledge ends in June 2026, so the details of Java 26 and 27 are not confirmed. Confirm the versions mise offers with `mise ls-remote java`.

Temurin is the Eclipse Adoptium build of OpenJDK. It is free and the usual default.

## Q3: the mise command

Question: what is the mise command for this?

Answer, inside the repo directory:

```bash
mise use java@temurin-25
```

The command installs the JDK when it is missing and writes `.mise.toml` in the repo with `java = "temurin-25"`. Commit that file so the version is pinned for the project. Then check it:

```bash
mise where java      # the path to add in IntelliJ (Q1, step 2)
java -version
```

If `mise use` cannot find `temurin-25`, list the names with `mise ls-remote java | grep temurin` and use the exact name printed.

How sure: from memory, not run on Jim's machine.

## Q4: `mise where java` says java@27.0.1.0.2 is not installed

Jim ran `mise where java` in the new repo. Output, exactly:

```
$ mise where java
mise ERROR java@27.0.1.0.2 not installed
mise ERROR Run with --verbose or MISE_VERBOSE=1 for more information
```

Cause: a mise config that applies in this directory already names `java@27.0.1.0.2`, probably the global config `~/.config/mise/config.toml` or a parent directory's `.mise.toml`. That version is not installed, and `mise use` has not yet been run in this repo, so `mise where` reports the missing one.

Fix: run `mise use java@temurin-25` in the repo. The command installs the JDK and writes the repo's `.mise.toml`, which overrides the global setting for this directory. Then `mise where java` prints the install path. To see which file names Java 27, run `mise config ls` or `mise ls --current`.

How sure: the cause is inferred from the error text and mise's usual config order. Jim's output decides it.

## Open questions

(Jim has more questions to add.)
