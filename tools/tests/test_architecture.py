"""Behavioral regressions for production ownership and borrowed UI contracts."""

import importlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
with patch.object(sys, 'path', [str(TOOLS), *sys.path]):
    architecture = importlib.import_module('architecture')


class ArchitectureTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='lume-architecture-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.write('src/lib.rs', 'mod app; mod ui; mod service;')
        self.write('src/app.rs', 'pub struct App { value: bool }')
        self.write('src/ui.rs', '')
        self.write('src/service.rs', '')

    def write(self, name, source):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source)

    def errors(self):
        return architecture.violations(self.root)

    def assert_cycle(self):
        errors = self.errors()
        self.assertTrue(any('production cycle' in error for error in errors),
                        errors)

    def test_direct_and_grouped_paths_create_cycles(self):
        self.write('src/app.rs', 'use crate::{service::{self, Request}};')
        self.write('src/service.rs', 'use crate::{app::App}; struct Request;')
        self.assert_cycle()

    def test_aliases_reexports_and_self_group_are_resolved(self):
        self.write('src/app.rs',
                   'use crate::service as worker; fn f(_: worker::R) {}')
        self.write('src/service.rs',
                   'pub use crate::{ui::{self as screen, Token as R}};')
        self.write('src/ui.rs', 'use crate::app as owner; struct Token;')
        self.assert_cycle()

    def test_reexported_module_alias_keeps_original_owner(self):
        self.write('src/service.rs', 'pub use crate::app as owner;')
        self.write('src/ui.rs', 'use crate::service::owner::App as State;')
        self.assertTrue(any('App authority' in error
                            for error in self.errors()))

    def test_crate_root_reexport_cannot_hide_app_authority(self):
        self.write('src/lib.rs',
                   'mod app; mod ui; mod service; pub use app::App as State;')
        self.write('src/ui.rs', 'use crate::State as Hidden;')
        self.assertTrue(any('App authority' in error
                            for error in self.errors()))

    def test_grouped_alias_import_preserves_app_authority(self):
        self.write('src/ui.rs', 'use crate::{app::{App as Hidden}};')
        self.assertTrue(any('App authority' in error
                            for error in self.errors()))

    def test_reexport_cycle_retains_intermediate_module_dependency(self):
        self.write('src/app.rs',
                   'pub struct App; use crate::service::Backref;')
        self.write('src/service.rs', 'pub use crate::app::App as Backref;')
        self.assert_cycle()

    def test_glob_reexport_cannot_hide_app_authority(self):
        self.write('src/service.rs', 'pub use crate::app::*;')
        self.write('src/ui.rs', 'use crate::service::*; fn draw(_: &App) {}')
        self.assertTrue(any('App authority' in error
                            for error in self.errors()))

    def test_unused_transitive_glob_still_imports_app_authority(self):
        self.write('src/service.rs', 'pub use crate::app::*;')
        for source in ('use crate::app::*;', 'use crate::service::*;'):
            with self.subTest(source=source):
                self.write('src/ui.rs', source)
                self.assertTrue(any('App authority' in error
                                    for error in self.errors()))

    def test_glob_of_presentation_types_remains_accepted(self):
        self.write('src/service.rs', 'pub struct View; pub struct Palette;')
        self.write('src/ui.rs', 'use crate::service::*; fn draw(_: &View) {}')
        self.assertEqual(self.errors(), [])

    def test_glob_does_not_import_private_app_bindings(self):
        self.write('src/service.rs', '''
            use crate::app::App;
            struct Internal { app: App }
            pub struct View;
        ''')
        self.write('src/ui.rs', 'use crate::service::*; fn draw(_: &View) {}')
        self.assertEqual(self.errors(), [])

    def test_explicit_lume_library_paths_are_local(self):
        self.write('src/app.rs', 'use lume::service::Request;')
        self.write('src/service.rs', 'use lume::app::App; struct Request;')
        self.assert_cycle()

    def test_test_only_feedback_and_comments_are_accepted(self):
        self.write('src/app.rs', 'use crate::service::Request;')
        self.write('src/service.rs', '''
            struct Request;
            // use crate::app::App;
            const NOTE: &str = "crate::app::App";
            #[cfg(test)] mod tests { use crate::app::App; }
        ''')
        self.assertEqual(self.errors(), [])

    def test_out_of_line_test_context_is_inherited(self):
        self.write('src/app.rs', 'use crate::service::Request;')
        self.write('src/service.rs',
                   'struct Request; #[cfg(test)] mod tests;')
        self.write('src/service/tests.rs', 'use crate::app::App; mod child;')
        self.write('src/service/tests/child.rs', 'use crate::ui::draw;')
        self.assertEqual(self.errors(), [])

    def test_production_reachability_wins_over_test_alias(self):
        self.write('src/lib.rs', '''
            mod app; mod ui;
            #[cfg(test)] #[path = "shared.rs"] mod test_service;
            #[path = "shared.rs"] mod service;
        ''')
        self.write('src/app.rs', 'use crate::service::Request;')
        self.write('src/shared.rs', 'use crate::app::App; struct Request;')
        (self.root / 'src/service.rs').unlink()
        self.assert_cycle()

    def test_orphan_production_source_is_rejected(self):
        self.write('src/orphan.rs', 'use crate::app::App;')
        self.assertTrue(any('unreachable production' in error
                            for error in self.errors()))

    def test_missing_declared_module_fails_closed(self):
        (self.root / 'src/service.rs').unlink()
        with self.assertRaises(ValueError):
            self.errors()

    def test_inline_relative_and_path_override_edges(self):
        self.write('src/app.rs', '''
            mod inner { use super::super::service::Request; }
            #[path = "separate.rs"] mod other;
        ''')
        self.write('src/separate.rs', 'use crate::service::Request;')
        self.write('src/service.rs',
                   'use crate::app::inner::Thing; struct Request;')
        self.assert_cycle()

    def test_binary_and_library_roots_are_independent(self):
        self.write('src/lib.rs', 'mod service;')
        self.write('src/service.rs', 'use lume::app::App;')
        self.write('src/main.rs', 'mod app; mod ui;')
        self.write('src/app.rs', 'use crate::ui::draw;')
        self.write('src/ui.rs', 'use lume::service::Request;')
        self.assertEqual(self.errors(), [])

    def test_binary_cycle_is_rejected(self):
        self.write('src/main.rs', 'mod cli; mod configuration;')
        self.write('src/cli.rs', 'use crate::configuration::Config;')
        self.write('src/configuration.rs', 'use crate::cli::Command;')
        self.assert_cycle()

    def test_child_dependency_on_composition_root_is_rejected(self):
        self.write('src/lib.rs', 'mod app; mod ui; mod service; struct Root;')
        self.write('src/service.rs', 'use crate::Root;')
        self.assert_cycle()

    def test_build_script_path_is_inspected_independently(self):
        self.write('build.rs', '#[path = "src/revision.rs"] mod revision;')
        self.write('src/revision.rs', 'fn inspect() {}')
        self.assertEqual(self.errors(), [])

    def test_build_script_children_resolve_from_its_crate_root(self):
        self.write('build.rs', 'mod revision;')
        self.write('revision.rs', 'mod child;')
        self.write('revision/child.rs', 'pub fn inspect() {}')
        self.assertEqual(self.errors(), [])
        self.write('revision/child.rs', 'fn inspect(_: crate::Root) {}')
        self.write('build.rs', 'mod revision; struct Root;')
        self.assert_cycle()

    def test_private_app_fields_and_borrowed_view_are_accepted(self):
        self.write('src/app.rs',
                   'pub struct App { value: bool } pub struct View;')
        self.write('src/ui.rs', 'use crate::app::View; fn draw(_: &View) {}')
        self.assertEqual(self.errors(), [])

    def test_each_public_visibility_of_app_field_is_rejected(self):
        for visibility in ('pub', 'pub(crate)', 'pub(super)', 'pub(in crate)'):
            with self.subTest(visibility=visibility):
                self.write('src/app.rs',
                           f'pub struct App {{ {visibility} value: bool }}')
                self.assertIn('App root fields must remain private',
                              self.errors())

    def test_hidden_view_backreference_and_type_alias_are_rejected(self):
        for body in ('pub struct View { app: Box<crate::app::App> }',
                     'pub type View = crate::app::App;'):
            with self.subTest(body=body):
                self.write('src/service.rs', body)
                self.write('src/ui.rs', 'use crate::service::View;')
                self.assertTrue(any('App authority' in error
                                    for error in self.errors()))

    def test_default_generic_app_authority_is_rejected(self):
        self.write('src/service.rs', '''
            pub struct View<T = crate::app::App> { value: T }
        ''')
        self.write('src/ui.rs', 'use crate::service::View;')
        self.assertTrue(any('App authority' in error
                            for error in self.errors()))

    def test_public_view_retains_authority_through_private_field_types(self):
        self.write('src/service.rs', '''
            type Hidden = crate::app::App;
            struct Inner { app: Hidden }
            pub struct View { inner: Inner }
        ''')
        for source in ('use crate::service::View;', 'use crate::service::*;'):
            with self.subTest(source=source):
                self.write('src/ui.rs', source)
                self.assertTrue(any('App authority' in error
                                    for error in self.errors()))

    def test_app_to_ui_edge_alone_is_rejected(self):
        self.write('src/app.rs', 'use crate::ui::draw;')
        self.assertIn('App must not depend on UI', self.errors())

    def test_copy_getter_take_and_replace_through_aliases_are_rejected(self):
        for call in ('std::mem::take(&mut state.pending_g())',
                     'std::mem::take::<bool>(&mut state.pending_g())',
                     'take(&mut state.pending_g())',
                     'mem::replace(&mut state.pending_g(), false)',
                     'consume(&mut state.pending_g())'):
            with self.subTest(call=call):
                self.write('src/app.rs', f'''
                    use std::mem::{{self, take, take as consume}};
                    fn pending_g(&self) -> bool {{ false }}
                    fn key() {{ {call}; }}
                ''')
                self.assertTrue(any('Copy getter temporary' in error
                                    for error in self.errors()), self.errors())

    def test_concrete_fields_refcell_and_borrowed_getters_are_accepted(self):
        self.write('src/app.rs', '''
            fn editor(&mut self) -> &mut Editor { &mut self.editor }
            fn pending_g(&self) -> bool { false }
            fn key() {
                std::mem::take(&mut state.pending_g);
                std::mem::take(&mut *state.borrow_mut());
                std::mem::take(&mut *state.editor());
                std::mem::replace(&mut state.editor().cursor, 0);
                std::mem::take(&mut state.get_mut(state.pending_g()));
            }
        ''')
        self.assertEqual(self.errors(), [])

    def test_generic_getter_calls_are_copy_temporaries(self):
        self.write('src/app.rs', '''
            fn pending_g<T>(&self) -> bool { false }
            fn key() {
                std::mem::take(&mut state.pending_g::<Option<bool>>());
            }
        ''')
        self.assertTrue(any('Copy getter temporary' in error
                            for error in self.errors()))

    def test_scalar_getter_with_where_clause_is_a_copy_temporary(self):
        self.write('src/app.rs', '''
            fn pending_g<T>(&self) -> bool where T: Copy { false }
            fn key() {
                core::mem::replace(&mut state.pending_g::<bool>(), false);
            }
        ''')
        self.assertTrue(any('Copy getter temporary' in error
                            for error in self.errors()))

    def test_mem_globs_and_transitive_reexports_are_checked(self):
        self.write('src/service.rs', 'pub use core::mem::*;')
        for source in ('use std::mem::*;', 'use crate::service::*;',
                       'use crate::service::take as consume;'):
            with self.subTest(source=source):
                operation = 'consume' if 'consume' in source else 'take'
                self.write('src/app.rs', f'''
                    {source}
                    fn pending_g(&self) -> bool {{ false }}
                    fn key() {{ {operation}(&mut state.pending_g()); }}
                ''')
                self.assertTrue(any('Copy getter temporary' in error
                                    for error in self.errors()), self.errors())

    def test_real_repository_contract(self):
        self.assertEqual(architecture.violations(TOOLS.parent), [])


if __name__ == '__main__':
    unittest.main()
