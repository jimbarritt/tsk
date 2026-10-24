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

Hidden folders in the file picker: the path is under `.local`, a hidden folder. In the macOS file dialog press Cmd+Shift+. (full stop) to show or hide hidden folders. Or press Cmd+Shift+G, or type `/`, and paste the path to go straight to it. Some IntelliJ dialogs also have a "Show hidden files" toggle in the toolbar. Pasting the path is the quickest.

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

## Result on Jim's machine: Java 25 under mise (checked, 2026-10-24)

After `mise use java@temurin-25` in `~/projects/java-dojo`, Jim ran the checks. Output, exactly:

```
$ mise where java
/Users/jmdb/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS
$ java --version
openjdk 25.0.5 2026-10-20 LTS
OpenJDK Runtime Environment Temurin-25.0.5+8 (build 25.0.5+8-LTS)
OpenJDK 64-Bit Server VM Temurin-25.0.5+8 (build 25.0.5+8-LTS, mixed mode, sharing)
$ javac --version
javac 25.0.5
```

The terminal uses Temurin 25.0.5, an LTS build dated 2026-10-20. The path to add in IntelliJ (Q1, step 2) is `/Users/jmdb/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS`. The directory name contains a `+`, which IntelliJ accepts. After a patch update (`25.0.6` for example) the directory name changes and the SDK must be added again.

Next step: add the SDK in IntelliJ by that path, set the project SDK and the Gradle JVM, then check that `./gradlew --version` prints the same Java.

## Q5: Kotlin or Groovy for the Gradle build

Question: Jim prefers Kotlin. Does the Kotlin DSL add complications to the setup?

Answer: no real complications for a Java project. Choose Kotlin (`build.gradle.kts`).

- The Kotlin DSL is the default for new builds in the Gradle `init` task and the IntelliJ wizard in current versions. The Gradle documentation uses it first.
- The Java source is unaffected. Only the build script is Kotlin. No Kotlin plugin, Kotlin standard library or Kotlin source directory is needed for a Java project.
- IntelliJ gives type-checked completion and navigation in `.kts` files, which Groovy scripts do not get to the same degree. This is the main benefit.
- Costs: the first run after a change to the build script compiles the script, so it is a few seconds slower than Groovy. Older tutorials and Stack Overflow answers show Groovy, so the syntax needs translating. For example `implementation 'x:y:1'` becomes `implementation("x:y:1")`, and `test { useJUnitPlatform() }` becomes `tasks.test { useJUnitPlatform() }`.

Settings that follow from the choice: `settings.gradle.kts`, `build.gradle.kts`, and the toolchain block in Q1, step 5.

How sure: from memory of Gradle and IntelliJ behaviour; not run on Jim's machine.

## Q6: the New Project wizard (checked from Jim's screenshot, 2026-10-24)

Question: should Name be `java-dojo` with Location pointing at the `java-dojo` directory, given that the wizard may nest `java-dojo/java-dojo`?

Answer: the wizard creates the project at Location/Name. Set Location to the parent directory. The wizard prints the final path under the Location field, so check that line.

Jim's settings:

| Field | Value |
|---|---|
| Template | Java |
| Name | `java-dojo` |
| Location | `~/Code/github/jimbarritt/` (the parent) |
| Printed path | "Project will be created in: ~/Code/github/jimbarritt/java-dojo" |
| Create Git repository | unticked (the repo exists) |
| Build system | Gradle |
| JDK | Eclipse Temurin 25.0.5 - aarch64, path under `/Users/jmdb/.local/share/mise/` |
| Gradle DSL | Kotlin or Groovy: check which button is selected before Create |
| Add sample code | unticked |

Findings:

- The printed path has no repeated name, so there is no nested `java-dojo/java-dojo`.
- IntelliJ listed the mise JDK in the JDK field with a link icon, without being given the path (Q1, step 2). So IntelliJ finds JDKs that mise installs. Adding the SDK by path is the fallback when it does not.
- The directory is not empty (`.git` and the files added so far), so IntelliJ can ask to confirm. Accept.
- Advanced Settings holds the group ID and artifact ID. For a practice repo any value works, for example group `dojo` and artifact `java-dojo`.
- Add sample code adds a `Main` class. Leave it unticked for a repo where Jim writes everything by hand.

After Create: run `./gradlew --version` in the terminal. It should print Java 25.0.5. If Gradle reports an unsupported Java version, the wizard chose an older Gradle (Q2).

## Q7: Gradle version in the wizard (checked from Jim's second screenshot, 2026-10-24)

The Advanced Settings of the wizard showed:

| Field | Value |
|---|---|
| Gradle DSL | Kotlin (Jim confirmed) |
| Gradle distribution | Wrapper |
| Gradle version | 8.5 (default offered), Auto-select unticked |
| Use these settings for future projects | ticked |
| GroupId | `org.ixcode` |
| ArtifactId | `java-dojo` |

Finding: Gradle 8.5 supports Java up to 21. A Java 25 project needs Gradle 9.1 or later (from memory, not run). The wizard offered 8.5 with Java 25 selected, so Create with the defaults gives a build that fails with an unsupported-version error.

Fix: pick the newest 9.x in the Gradle version dropdown before Create, or tick Auto-select. After Create, run `./gradlew --version` in the terminal. It should print Gradle 9.x and Java 25.0.5. An existing project changes version with `./gradlew wrapper --gradle-version <9.x>`, run with a Java the old Gradle supports.

Jim's choice: Gradle 9.8.0, picked in the wizard dropdown. It is newer than the 9.1 floor above. The result of `./gradlew --version` after Create is still to be recorded.

How sure: the Gradle and Java compatibility is from memory. Jim's `./gradlew --version` output decides.

## Q8: the generated .gitignore does not ignore the whole .idea directory

Jim noticed the generated `.gitignore` has an IntelliJ section that lists only `.idea/modules.xml`, `.idea/jarRepositories.xml`, `.idea/compiler.xml`, `.idea/libraries/`, `*.iws`, `*.iml` and `*.ipr`. The project is up and running (Jim, 2026-10-24).

Answer: `.idea` has two kinds of file. The template ignores the ones IntelliJ regenerates from the Gradle build. It leaves the ones JetBrains says can be shared, such as `misc.xml`, `gradle.xml`, code style settings, inspection profiles and shared run configurations. IntelliJ also writes `.idea/.gitignore`, which ignores `workspace.xml`, `shelf/` and similar personal files.

For a solo practice repo, ignore the whole directory. The shared files contain absolute paths from the machine, such as `/Users/jmdb/...`, and nobody else needs them. IntelliJ rebuilds the settings from `build.gradle.kts` when the repo is opened. Add this line to `.gitignore`:

```
.idea/
```

If `.idea` is already committed, untrack it and keep the files:

```bash
git rm -r --cached .idea
git commit -m "Stop tracking .idea"
```

How sure: from memory of the JetBrains guidance and the template. Jim can confirm with `cat .idea/.gitignore` and `git status`.

## Q9: Vim keybindings in IntelliJ

Question: can IntelliJ use Vim keybindings, so that `:w` saves?

Answer: yes, with the IdeaVim plugin from JetBrains. It supports modes, motions, registers, macros, `/` search, and `:w`, `:q` and `:wq`.

Install: Settings (Cmd+,), Plugins, Marketplace, search for IdeaVim, Install, restart the IDE. The editor then starts in normal mode: `i` inserts, `Esc` leaves insert mode, and `:w` saves.

Points:

- IntelliJ also autosaves on focus change and on build, so `:w` is mostly habit.
- IdeaVim reads `~/.ideavimrc`. It supports many `.vimrc` options, not all. Options such as `set relativenumber` and `set ideajoin` go there.
- The Tools menu, Vim Emulator, turns the emulation on and off.
- On first use of a clashing Ctrl key (Ctrl+C, Ctrl+V and similar), IdeaVim asks whether the key goes to the IDE or to Vim. The choice is per key.
- For the interview: check whether the coding platform supports Vim keybindings. Some browser editors have a keybinding setting and some do not. Practising with Vim bindings helps only if they are available on the day.

How sure: from memory of IdeaVim; not run on Jim's machine.

## Q10: red `System` and "project JDK is misconfigured" after Create (2026-10-24)

What Jim saw: the generated `Main.java` had a red underline under `System` and a red error marker. After an IDE restart a banner said the project JDK is misconfigured.

`Main.java` itself is valid Java 25. The template uses `static void main()` with no `public` and no arguments, which Java 25 allows.

Facts from Jim's machine:

- In a terminal, `mise where java` prints `/Users/jmdb/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS`, and `bin/java` exists directly under it.
- In IntelliJ, the Temurin 25 SDK entry has the JDK home `/Users/jmdb/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS/Contents/Home`.

Cause: the entry that IntelliJ registered through its automatic detection of the mise JDK (the link icon in the wizard, Q6) has a path ending in `Contents/Home`. That is the layout of a macOS JDK package. The mise install has its `bin` directly under the install directory, so the stored path does not match it. IntelliJ then cannot read the class library, and `System` does not resolve. This reverses the finding in Q6, where the detection looked like it worked.

Fix: remove the entry in Project Structure, SDKs. Add JDK from disk, and select the install directory without `Contents/Home`. Set the project SDK to it and the language level to 25. Set the Gradle JVM to it too, then reload the Gradle project.

How sure: the symptom and the path difference are checked. Whether `Contents/Home` exists on disk was not checked. Jim's result after the fix decides.

## Open questions

(Jim has more questions to add.)
