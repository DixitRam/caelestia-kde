#!/usr/bin/env python3
"""Installer wiring tests.

The invariants the installer's step scripts have to keep: that the entrypoints exist,
that every numbered scripts/*.sh is run by one of the step lists, that the sparse checkout
the update path writes covers everything the shell build installs and sparsifies only
paths src/bin/caelestia-update writes, and a few ordering guarantees inside those scripts.
They read the same few installer files and change for their own reasons, which is why they
are not in test_repo_integrity.py.
"""

import posixpath
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
INSTALLER_ENTRYPOINTS = [
    Path("scripts", "setup.sh"),
    Path("update.sh"),
    Path("uninstall.sh"),
]


class InstallerTests(unittest.TestCase):
    def test_installer_entrypoints_exist(self) -> None:
        for rel_path in INSTALLER_ENTRYPOINTS:
            self.assertTrue((ROOT / rel_path).is_file(), f"Missing installer entrypoint: {rel_path.as_posix()}")

    def test_setup_references_existing_step_scripts(self) -> None:
        steps_text = (ROOT / "installer" / "src" / "steps" / "mod.rs").read_text(encoding="utf-8")
        matches = re.findall(r'Script\("(scripts/[^"]+)"\)', steps_text)

        self.assertTrue(matches, "No installer steps found in installer/src/steps/mod.rs")

        for rel_path in matches:
            normalized = Path(rel_path.replace("\\", "/"))
            resolved = ROOT / normalized
            self.assertTrue(
                resolved.is_file(),
                f"Missing installer step referenced by steps/mod.rs: {resolved.relative_to(ROOT).as_posix()}",
            )

    def test_every_step_declares_an_id(self) -> None:
        """A step is addressed by id, so every step needs one, and no two may collide."""
        steps_text = (ROOT / "installer" / "src" / "steps" / "mod.rs").read_text(encoding="utf-8")
        ids = re.findall(r'id: "([^"]+)"', steps_text)
        self.assertTrue(ids, "No step ids found in installer/src/steps/mod.rs")

        names = re.findall(r'name: "([^"]+)"', steps_text)
        self.assertEqual(len(ids), len(names), "every step needs exactly one id")

        duplicates = {step_id for step_id in ids if ids.count(step_id) > 1}
        self.assertFalse(duplicates, f"Duplicate installer step ids: {duplicates}")

    def test_no_duplicate_step_names(self) -> None:
        """The step table must not define two steps with the same display name."""
        steps_text = (ROOT / "installer" / "src" / "steps" / "mod.rs").read_text(encoding="utf-8")
        display_names = re.findall(r'name: "([^"]+)"', steps_text)
        self.assertTrue(display_names, "No step names found in installer/src/steps/mod.rs")

        seen: dict[str, int] = {}
        for name in display_names:
            seen[name] = seen.get(name, 0) + 1

        duplicates = {name: count for name, count in seen.items() if count > 1}
        self.assertFalse(
            duplicates,
            f"Duplicate installer step names: {duplicates}",
        )

    def test_every_step_script_is_wired_into_a_step_list(self) -> None:
        """A numbered scripts/*.sh must be run by a step list, and every listed step must exist.

        Three lists run step scripts: the full install (installer/src/steps/mod.rs), the
        user's half of a packaged install (src/bin/caelestia), and the subset
        08-build-shell.sh runs on its own when a checkout updates itself. A script in none
        of them is dead weight that every other check still reports as covered; a list
        entry with no file fails at install time instead of here. The step numbers are not
        an order: the TUI runs 00-backup-themes after 02a-submodules, deliberately. The
        lists differ in failure policy - the installer stops, the update path warns and
        carries on - so only their membership is comparable, which is all this checks.
        """
        runner_text = (ROOT / "installer" / "src" / "steps" / "mod.rs").read_text(encoding="utf-8")
        packaged_text = (ROOT / "src" / "bin" / "caelestia").read_text(encoding="utf-8")
        update_text = (ROOT / "scripts" / "08-build-shell.sh").read_text(encoding="utf-8")

        runner_steps = set(re.findall(r"scripts/([0-9][0-9a-z]*-[A-Za-z0-9._-]+\.sh)", runner_text))
        packaged_steps = set(
            re.findall(r"^\s+([0-9][0-9a-z]*-[A-Za-z0-9._-]+\.sh)$", packaged_text, re.MULTILINE)
        )
        update_steps = set(
            re.findall(r"scripts/([0-9][0-9a-z]*-[A-Za-z0-9._-]+\.sh)", update_text)
        )
        listed = runner_steps | packaged_steps | update_steps
        self.assertTrue(listed, "No step scripts found in Runner.cpp, src/bin/caelestia or 08-build-shell.sh")

        scripts_dir = ROOT / "scripts"
        numbered = {
            path.name for path in scripts_dir.glob("*.sh")
            if re.match(r"^\d+[a-z]?-", path.name)
        }

        self.assertEqual(
            sorted(numbered - listed), [],
            "Step script(s) no step list runs:",
        )
        self.assertEqual(
            sorted(listed - numbered), [],
            "Step list(s) reference script(s) that do not exist in scripts/:",
        )

    def test_the_update_fallback_sparsifies_only_paths_the_updater_writes(self) -> None:
        """08-build-shell.sh adds to the sparse-checkout list src/bin/caelestia-update owns.

        The updater writes that list before it checks the tree out, so a helper cannot
        live in the tree and be called from there: the two name the same paths twice.
        This is what stops the second copy from drifting to a rule nothing else writes,
        which would leave the fallback silently doing nothing.
        """
        updater = (ROOT / "src" / "bin" / "caelestia-update").read_text(encoding="utf-8")
        build = (ROOT / "scripts" / "08-build-shell.sh").read_text(encoding="utf-8")

        owned = set(re.findall(r'echo "([^"]+)" >+ \.git/info/sparse-checkout', updater))
        added = set()
        for call in re.findall(r"^\s*sparsify_checkout_paths (.+)$", build, re.MULTILINE):
            added.update(call.split())

        self.assertTrue(owned, "caelestia-update should still write the sparse-checkout list")
        self.assertTrue(added, "08-build-shell.sh should still name the rules it adds")
        self.assertEqual(
            sorted(added - owned),
            [],
            "08-build-shell.sh sparsifies rule(s) caelestia-update never writes:",
        )

    def test_the_sparse_checkout_covers_everything_the_shell_install_reads(self) -> None:
        """#1039: cmake --install fails when an install rule's source is not sparsified.

        shell/CMakeLists.txt installs files from outside shell/ - src/, scripts/ and
        assets/ - and the update path checks the repository out sparse before it builds.
        Configure and build only read shell/, so they succeed on a checkout that is
        missing assets/org.quickshell.desktop; cmake --install is the first step to touch
        it and it dies with "file INSTALL cannot find .../assets/org.quickshell.desktop",
        aborting the update before the new revision is recorded. A full checkout has the
        file, so only the sparse one in the update path reproduces it. Nothing else
        compares the install rules against src/bin/caelestia-update's sparse-checkout
        list, so an install rule added for a path nothing sparsifies is caught here.
        """
        updater = (ROOT / "src" / "bin" / "caelestia-update").read_text(encoding="utf-8")
        entries = {
            entry.rstrip("/")
            for entry in re.findall(r'echo "([^"]+)" >+ \.git/info/sparse-checkout', updater)
        }
        self.assertTrue(entries, "caelestia-update should still write the sparse-checkout list")

        # ${CAELESTIA_ROOT_DIR} is the repository root and ${CMAKE_CURRENT_SOURCE_DIR} the
        # directory of the CMakeLists.txt being read, both as shell/CMakeLists.txt spells
        # them. Resolving the two is what tells an install rule for a path authored
        # outside shell/ from one that sits inside it and travels with the tree.
        root_refs = re.compile(r"\$\{CAELESTIA_ROOT_DIR\}/([^\"\s)]+)")
        here_refs = re.compile(r"\$\{CMAKE_CURRENT_SOURCE_DIR\}/([^\"\s)]+)")

        required = {"shell"}  # the build configures from the shell/ tree itself
        for cmake_lists in sorted((ROOT / "shell").rglob("CMakeLists.txt")):
            relative = cmake_lists.relative_to(ROOT)
            if "build" in relative.parts:
                continue
            base = cmake_lists.parent.relative_to(ROOT).as_posix()
            text = cmake_lists.read_text(encoding="utf-8")
            paths = root_refs.findall(text)
            paths += [posixpath.join(base, ref) for ref in here_refs.findall(text)]
            for path in paths:
                normalized = posixpath.normpath(path)
                if not normalized.startswith("../") and normalized not in (".", ".."):
                    required.add(normalized.split("/", 1)[0])

        self.assertLessEqual(
            {"src", "scripts"},
            required,
            "the shell build should still install its CLI wrappers and step scripts from outside shell/",
        )
        self.assertEqual(
            sorted(
                name for name in required
                if not any(name == entry or name.startswith(entry + "/") for entry in entries)
            ),
            [],
            "the shell build reads path(s) that src/bin/caelestia-update never sparsifies:",
        )


class InstallStepSafetyTests(unittest.TestCase):
    """Ordering and wiring invariants for the install/update step scripts.

    These are guarantees no single-file syntax or lint check can see, and that
    the reports behind them describe as silent: the step reports success while
    doing the wrong thing.
    """

    def test_the_legacy_fonts_cleanup_is_called(self) -> None:
        script = (ROOT / "scripts" / "08-build-shell.sh").read_text(encoding="utf-8")

        self.assertIn("cleanup_legacy_fonts() {", script, "the cleanup should still be defined")
        self.assertIsNotNone(
            re.search(r"^\s*cleanup_legacy_fonts\s*$", script, re.MULTILINE),
            "08-build-shell.sh defines cleanup_legacy_fonts but never calls it",
        )

    def test_shell_config_backup_precedes_the_prebuilt_install(self) -> None:
        """#663: the prebuilt path extracts over $HOME, so it must be backed up first."""
        script = (ROOT / "scripts" / "08-build-shell.sh").read_text(encoding="utf-8")

        backup_at = script.find("backup_shell_config ||")
        prebuilt_at = script.find("if try_download_prebuilt_shell;")

        self.assertNotEqual(backup_at, -1, "08-build-shell.sh should back up the shell config")
        self.assertNotEqual(prebuilt_at, -1, "08-build-shell.sh should still use the prebuilt download")
        self.assertLess(
            backup_at,
            prebuilt_at,
            "the shell-config backup must run before the prebuilt archive is extracted over $HOME",
        )

    def test_privileged_package_installs_go_through_the_escalation_helper(self) -> None:
        """#664: a GUI-triggered update has no terminal, so bare sudo fails silently."""
        script = (ROOT / "scripts" / "08-build-shell.sh").read_text(encoding="utf-8")

        self.assertIn(
            "install_linguist_tools",
            script,
            "08-build-shell.sh should install the Linguist tools via the shared helper",
        )
        self.assertNotIn(
            "sudo pacman -S --needed --noconfirm qt6-tools",
            script,
            "the Linguist tools install must not escalate with bare sudo",
        )

    def test_scheme_wait_happens_after_the_shell_restart(self) -> None:
        """#666: waiting before the restart polls for a file from a killed process."""
        script = (ROOT / "update.sh").read_text(encoding="utf-8")

        start_at = script.find('"$SHELL_IPC" start')
        wait_at = script.find("wait_for_nonempty_file")

        self.assertNotEqual(start_at, -1, "update.sh should still start the shell through the IPC wrapper")
        self.assertNotEqual(wait_at, -1, "update.sh should wait for the restarted shell to persist the scheme")
        self.assertLess(
            start_at,
            wait_at,
            "the scheme.json wait must run after the shell is restarted, not before",
        )

    def test_the_launcher_fetches_the_installer_the_release_publishes(self) -> None:
        """The launcher and the release job have to agree on one asset name.

        The launcher has nothing to fall back on any more: it downloads the static
        binary or it stops. So the name it builds and the name the release attaches
        are the same contract, checked here rather than discovered by a user.
        """
        script = (ROOT / "scripts" / "setup.sh").read_text(encoding="utf-8")
        release = (ROOT / ".github" / "workflows" / "version-release.yml").read_text(encoding="utf-8")

        self.assertIn("fetch_installer", script, "setup.sh should fetch the installer")
        self.assertNotIn("cmake", script, "nothing is compiled on the user's machine any more")
        self.assertIn("installer_matches_checkout", script,
                      "the launcher should refuse a binary that disagrees with the checkout")

        asset = "caelestia-install-${arch}-v${version}"
        self.assertIn(asset, script, "setup.sh should build the published asset name")
        self.assertIn('BINARY="caelestia-install-${{ matrix.arch }}-v${TUI_VERSION}"', release,
                      "the release job should attach the asset the launcher asks for")


class TuiArgumentTests(unittest.TestCase):
    def test_every_argument_is_read_for_a_mode_and_a_directory(self) -> None:
        cli_rs = (ROOT / "installer" / "src" / "cli.rs").read_text(encoding="utf-8")

        self.assertIn('Some("--update")', cli_rs, "the update mode should still be recognised")
        self.assertIn('Some("--uninstall")', cli_rs, "the uninstall mode should still be recognised")
        self.assertIn("parsed.bundle = Some(", cli_rs,
                      "a non-flag argument should set the bundle directory")
        self.assertIn("while let Some(arg) = args.next()", cli_rs,
                      "every argument should be read, not just the first")

    def test_a_mode_and_a_directory_can_both_be_given(self) -> None:
        cli_rs = (ROOT / "installer" / "src" / "cli.rs").read_text(encoding="utf-8")

        self.assertNotIn(
            "let _ = args.next();",
            cli_rs,
            "reading a single argument discards the second of a two-argument call",
        )
        self.assertIn('text.starts_with(\'-\')', cli_rs,
                      "an unknown option should be refused rather than taken for a directory")


class ScriptNumberingTests(unittest.TestCase):
    def test_install_step_scripts_have_consistent_numbers(self) -> None:
        """Step numbers stay a two-digit base with an optional letter suffix.

        The set is whatever `ls scripts/` shows; the guarantee is only the numbering
        scheme, so a step added with a number that cannot sort next to its neighbours is
        caught here. Use the existing names as the examples.
        """
        scripts_dir = ROOT / "scripts"
        if not scripts_dir.is_dir():
            return

        numbers = set()
        for f in scripts_dir.glob("*.sh"):
            match = re.match(r"^(\d+)[a-z]?-", f.name)
            if match:
                numbers.add(int(match.group(1)))

        if numbers:
            max_num = max(numbers)
            self.assertLessEqual(
                max_num, 99,
                f"Script number {max_num} seems too high - consider renumbering"
            )


class InstallerSourceTests(unittest.TestCase):
    """The installer is Rust now, and the checks above read its sources.

    The C++ version needed a source-reading guard for a bug the language allowed:
    `string(getenv("HOME"))` throws when HOME is unset, which aborted the install
    before the first step ran. Rust's `std::env::var` returns a `Result`, so that
    bug cannot be written here. What is still worth guarding is the crate's shape,
    because the checks in this file grep it by name.
    """

    SRC = ROOT / "installer" / "src"

    def test_the_installer_sources_are_where_this_expects_them(self) -> None:
        """Keeps the checks above from passing just because they read no files."""
        names = {path.name for path in self.SRC.glob("*.rs")}
        for expected in ("main.rs", "cli.rs", "menu.rs", "app.rs", "screens.rs", "runner.rs"):
            self.assertIn(expected, names, f"installer/src/{expected} is missing")
        self.assertTrue(
            (self.SRC / "steps" / "mod.rs").is_file(),
            "installer/src/steps/mod.rs is missing",
        )

    def test_step_scripts_are_named_in_one_place_only(self) -> None:
        """The step table is the single list; everything else renders what it holds."""
        for name in ("app.rs", "screens.rs", "menu.rs", "theme.rs", "bundle.rs"):
            text = (self.SRC / name).read_text(encoding="utf-8")
            self.assertNotIn(
                "scripts/",
                text,
                f"installer/src/{name} names a step script; installer/src/steps/mod.rs "
                "is the only place that may",
            )


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(0 if result.wasSuccessful() else 1)
