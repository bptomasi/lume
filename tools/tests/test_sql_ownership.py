"""Exercise SQL ownership failures with isolated Rust source trees."""

import importlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
with patch.object(sys, 'path', [str(TOOLS), *sys.path]):
    ownership = importlib.import_module('sql_ownership')


class SqlOwnershipTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='lume-sql-guard-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'src/store').mkdir(parents=True)
        self.write('src/lib.rs', 'mod store;')
        self.write('src/store/mod.rs', '')

    def write(self, name, source):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source)

    def inspect(self, source):
        self.write('src/store/mod.rs', source)
        return ownership.violations(self.root)

    def test_each_statement_and_fragment_fails_even_on_one_line(self):
        literals = [
            'SELECT 1', 'select id from articles', 'PRAGMA user_version',
            'INSERT INTO settings VALUES (?1, ?2)',
            'UPDATE articles SET unread = ?1', 'DELETE FROM undo_log',
            'CREATE TABLE sample (id INTEGER)', 'DROP TABLE sample',
            'BEGIN IMMEDIATE', 'COMMIT', 'ROLLBACK',
            'WITH matching AS ({predicate}) SELECT * FROM matching',
            '{base} WHERE a.id = ?1', '{} AND ({})', 'a.unread = 1',
        ]
        for literal in literals:
            with self.subTest(literal=literal):
                errors = self.inspect(f'const SQL: &str = r#"{literal}"#;')
                self.assertEqual(len(errors), 1, errors)
                self.assertIn('inline store SQL', errors[0])

    def test_sql_methods_catch_unrecognized_and_fragmented_arguments(self):
        for method in ownership.SQL_METHODS:
            with self.subTest(method=method):
                errors = self.inspect(f'fn read() {{ db.{method}("x", []); }}')
                self.assertEqual(len(errors), 1, errors)

    def test_raw_multiline_escaped_and_byte_queries_fail(self):
        for literal in [
            'r###"SELECT id\nFROM articles"###',
            '"SELECT id\\nFROM articles"', 'b"PRAGMA user_version"',
            '"\\x53ELECT 1"',
        ]:
            with self.subTest(literal=literal):
                self.assertTrue(self.inspect(f'const SQL: &str = {literal};'))

    def test_comments_strings_and_api_values_are_not_source_context(self):
        errors = self.inspect('''
            // const SQL: &str = "SELECT * FROM articles";
            /* nested /* "PRAGMA user_version" */ SQL example */
            const MESSAGE: &str = "Select a passage first";
            const ATTRIBUTE: &str = "#[cfg(test)]";
            fn configure() { db.pragma_update(None, "journal_mode", "DELETE"); }
            fn lifetime<'a>(text: &'a str) { let brace = '}'; }
        ''')
        self.assertEqual(errors, [])
        self.assertTrue(self.inspect('''
            const ATTRIBUTE: &str = "#[cfg(test)]";
            const SQL: &str = "SELECT 1";
        '''))

    def test_inline_test_items_are_allowed_but_siblings_still_fail(self):
        test_source = '''
            #[cfg(test)]
            mod tests {
                const SQL: &str = "SELECT 1";
                const FAULT: &str = include_str!("sql/test_fault.sql");
            }
            #[test]
            fn isolated() { let sql = "PRAGMA user_version"; }
        '''
        self.assertEqual(self.inspect(test_source), [])
        errors = self.inspect(test_source + '\nconst SQL: &str = "SELECT 1";')
        self.assertEqual(len(errors), 1, errors)

    def test_names_and_non_test_configurations_do_not_exempt_files(self):
        for attribute in ['', '#[cfg(not(test))]', '#[cfg(feature = "test")]']:
            with self.subTest(attribute=attribute):
                self.assertTrue(self.inspect(
                    attribute + '\nfn tests() { let sql = "SELECT 1"; }'
                ))
        self.write('src/store/test_helper.rs', 'const SQL: &str = "SELECT 1";')
        self.assertTrue(self.inspect(''))

    def test_flat_store_module_obeys_the_same_inline_sql_contract(self):
        (self.root / 'src/store/mod.rs').unlink()
        self.write('src/store.rs', 'const SQL: &str = "SELECT 1";')
        errors = ownership.violations(self.root)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('src/store.rs:1: inline store SQL', errors[0])

    def test_test_only_out_of_line_modules_inherit_context(self):
        self.write('src/store/checks.rs', '''
            mod helper;
            const SQL: &str = include_str!("sql/test_fault.sql");
        ''')
        self.write('src/store/checks/helper.rs',
                   'const SQL: &str = "SELECT 1";')
        self.assertEqual(self.inspect('#[cfg(test)] mod checks;'), [])
        self.assertTrue(self.inspect('mod checks;'))

    def test_path_attributes_and_production_aliases_keep_true_ownership(self):
        self.write('src/store/faults.rs', '''
            const SQL: &str = include_str!("sql/test_fault.sql");
        ''')
        source = '#[cfg(test)]\n#[path = "faults.rs"]\nmod tests;'
        self.assertEqual(self.inspect(source), [])
        source += '\n#[path = "faults.rs"]\nmod production;'
        errors = self.inspect(source)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('production includes test SQL', errors[0])

    def test_public_production_aliases_override_test_module_context(self):
        self.write('src/store/shared.rs', 'const SQL: &str = "SELECT 1";')
        test_module = '#[cfg(test)] #[path = "shared.rs"] mod checks;'
        for visibility in ['pub', 'pub(crate)', 'pub(in crate::store)']:
            for attributes in [
                '#[path = "shared.rs"] #[allow(dead_code)]',
                '#[allow(dead_code)] #[path = "shared.rs"]',
            ]:
                with self.subTest(visibility=visibility, attributes=attributes):
                    source = (f'{test_module}\n{attributes}\n'
                              f'{visibility} mod production;')
                    errors = self.inspect(source)
                    self.assertEqual(len(errors), 1, errors)
                    self.assertIn('shared.rs:1: inline store SQL', errors[0])

    def test_inline_module_paths_keep_inherited_context_and_aliases(self):
        self.write('src/store/checks/helper.rs',
                   'const SQL: &str = "SELECT 1";')
        source = '#[cfg(test)] mod checks { pub mod helper; }'
        self.assertEqual(self.inspect(source), [])
        source += '''
            mod production {
                #[path = "../checks/helper.rs"] pub mod shared;
            }
        '''
        (self.root / 'src/store/production').mkdir()
        errors = self.inspect(source)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('checks/helper.rs:1: inline store SQL', errors[0])

    def test_any_production_source_rejects_test_sql_and_misplaced_sql(self):
        for source in [
            'const SQL: &str = include_str!("sql/test_fault.sql");',
            'const SQL: &[u8] = include_bytes!(r#"sql/test_fault.sql"#);',
            'include!("sql/test_fault.sql");',
            'include_str!(concat!("sql/", "test_", "fault.sql"));',
        ]:
            with self.subTest(source=source):
                self.write('src/app/mod.rs', source)
                errors = ownership.violations(self.root)
                self.assertEqual(len(errors), 1, errors)
                self.assertIn('production includes test SQL', errors[0])
        self.write('src/app/mod.rs', 'include_str!("select_value.sql");')
        errors = ownership.violations(self.root)
        self.assertIn('leaves owner sql/', errors[0])

    def test_cross_owner_includes_fail_in_production_and_unit_tests(self):
        for attribute in ['', '#[cfg(test)]', '#[cfg(not(test))]']:
            with self.subTest(attribute=attribute):
                source = attribute + """
                    mod checks {
                        include_str!("../store/sql/select_articles.sql");
                    }
                """
                self.write('src/app/mod.rs', source)
                errors = ownership.violations(self.root)
                self.assertEqual(len(errors), 1, errors)
                self.assertIn('leaves owner sql/', errors[0])
        self.write('src/app/mod.rs', """
            #[cfg(test)] mod checks {
                include_str!("../store/sql/test_drop_article_search.sql");
            }
        """)
        errors = ownership.violations(self.root)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('leaves owner sql/', errors[0])

    def test_all_macro_delimiters_enforce_sql_ownership(self):
        for macro in ['include_str', 'include_bytes', 'include']:
            for opening, closing in [('(', ')'), ('[', ']'), ('{', '}')]:
                with self.subTest(macro=macro, opening=opening):
                    source = (f'{macro}!{opening}'
                              '"../app/sql/select_value.sql"'
                              f'{closing};')
                    errors = self.inspect(source)
                    self.assertEqual(len(errors), 1, errors)
                    self.assertIn('leaves owner sql/', errors[0])
        errors = self.inspect('''
            include_str![concat!{"sql/", "test_fault.sql"}];
        ''')
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('production includes test SQL', errors[0])

    def test_module_context_keeps_fixture_includes_inside_the_owner(self):
        self.write('src/app/mod.rs', '#[cfg(test)] mod checks;')
        self.write('src/app/checks.rs', """
            include_str!("sql/catalog_worker/test_fault.sql");
        """)
        self.assertEqual(ownership.violations(self.root), [])
        self.write('src/app/checks.rs', """
            include_str!("../store/sql/test_fault.sql");
        """)
        errors = ownership.violations(self.root)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('leaves owner sql/', errors[0])

    def test_normalization_prevents_traversal_outside_the_owner(self):
        self.assertEqual(self.inspect(
            'include_str!("sql/../sql/select_value.sql");'
        ), [])
        errors = self.inspect('include_str!("sql/../../app/sql/select.sql");')
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('leaves owner sql/', errors[0])
        self.write('src/store/sql/select.sql', 'SELECT 1')
        self.write('src/app/sql/select.sql', 'SELECT 1')
        link = self.root / 'src/store/sql/aliased.sql'
        link.symlink_to('../../app/sql/select.sql')
        self.assertTrue(self.inspect('include_str!("sql/aliased.sql");'))

    def test_symlinks_preserve_test_prefix_checks_in_both_directions(self):
        self.write('src/store/sql/select_value.sql', 'SELECT 1')
        self.write('src/store/sql/test_fault.sql', 'SELECT 1')
        (self.root / 'src/store/sql/test_alias.sql').symlink_to(
            'select_value.sql'
        )
        (self.root / 'src/store/sql/production_alias.sql').symlink_to(
            'test_fault.sql'
        )
        for name in ['test_alias.sql', 'production_alias.sql']:
            with self.subTest(name=name):
                errors = self.inspect(f'include_str!("sql/{name}");')
                self.assertEqual(len(errors), 1, errors)
                self.assertIn('production includes test SQL', errors[0])

    def test_manifest_concat_preserves_the_absolute_owner(self):
        valid = """include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/src/store/sql/select_value.sql"
        ));"""
        self.assertEqual(self.inspect(valid), [])
        errors = self.inspect(valid.replace('/src/store/', '/src/app/'))
        self.assertEqual(len(errors), 1, errors)
        self.assertIn('leaves owner sql/', errors[0])

    def test_unsupported_dynamic_sql_paths_fail_explicitly(self):
        for source in [
            'include_str!(concat!(env!("OUT_DIR"), "/schema.sql"));',
            'include_str!(concat!(custom!(), "/select.sql"));',
            'include_str!(path_macro!());',
        ]:
            with self.subTest(source=source):
                errors = self.inspect(source)
                self.assertEqual(len(errors), 1, errors)
                self.assertIn('SQL include path:', errors[0])
        self.assertEqual(self.inspect('''
            include_bytes!(concat!(env!("OUT_DIR"), "/reader-syntaxes.pack"));
        '''), [])

    def test_cli_fails_with_path_and_line_and_succeeds_after_extraction(self):
        self.inspect('\nconst SQL: &str = "SELECT 1";')
        command = [sys.executable, str(TOOLS / 'sql_ownership.py'),
                   '--root', str(self.root)]
        result = subprocess.run(command, text=True, capture_output=True,
                                check=False, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertIn('src/store/mod.rs:2:', result.stderr)
        self.inspect('const SQL: &str = include_str!("sql/select_value.sql");')
        result = subprocess.run(command, text=True, capture_output=True,
                                check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unclosed_source_or_missing_source_tree_fails_closed(self):
        self.write('src/store/mod.rs', 'const SQL: &str = r#"SELECT 1;')
        with self.assertRaisesRegex(ValueError, 'Unclosed raw string'):
            ownership.violations(self.root)
        with self.assertRaisesRegex(ValueError, 'No source directory'):
            ownership.violations(self.root / 'missing')

    def test_guard_runs_in_root_make_check(self):
        makefile = (TOOLS.parent / 'Makefile').read_text()
        check = makefile.split('\ncheck:\n', 1)[1].split('\n\n', 1)[0]
        self.assertIn('$(MAKE) check-sql', check)
        self.assertIn('$(PYTHON) tools/sql_ownership.py', makefile)


if __name__ == '__main__':
    unittest.main()
