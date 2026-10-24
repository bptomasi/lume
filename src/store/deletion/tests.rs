//! Check all ownership edges, not just visibility of the parent article.

use crate::store::Store;
use anyhow::Result;
use std::sync::atomic::AtomicBool;

#[test]
fn configuration_creation_never_reuses_a_deleted_folder_undo_identity()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let original = store.create_collection("Original")?;
    store.clear_history()?;
    store.delete_collection(original)?;
    store.connection.execute(
        include_str!("../sql/configuration/insert_folder.sql"),
        ["External"],
    )?;
    let external = store.connection.last_insert_rowid();
    assert_ne!(
        original, external,
        "A pending undo must not own a new folder"
    );
    assert!(store.undo()?);
    let names = store
        .library()?
        .collections
        .into_iter()
        .map(|item| item.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&"Original".into()));
    assert!(names.contains(&"External".into()));
    Ok(())
}

fn fixture() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.connection.execute_batch(include_str!(
        "../sql/deletion/test_setup_cascades.sql"
    ))?;
    Ok((directory, store))
}

#[test]
fn saved_query_identities_are_not_reused_after_deletion() -> Result<()> {
    let (_directory, store) = fixture()?;
    store.save_smart_list("Original", &Default::default())?;
    let smart = store.smart_lists()?.remove(0).id;
    store.delete_smart_list(smart)?;
    store.save_smart_list("Replacement", &Default::default())?;
    assert!(store.smart_lists()?.remove(0).id > smart);
    store.save_note_collection("Original", "")?;
    let notes = store.note_collections()?.remove(0).id;
    store.delete_note_collection("Original")?;
    store.save_note_collection("Replacement", "")?;
    assert!(store.note_collections()?.remove(0).id > notes);
    Ok(())
}

#[test]
fn permanent_deletion_cascades_every_owner_and_keeps_unrelated_data()
-> Result<()> {
    let (_directory, store) = fixture()?;
    assert!(store.purge_deleted(1)?);
    let mut statement = store
        .connection
        .prepare(include_str!("../sql/deletion/test_select_cascades.sql"))?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    statement.query_row([], |row| {
        for (index, name) in columns.iter().enumerate() {
            assert!(row.get::<_, bool>(index)?, "Broken cascade: {name}");
        }
        Ok(())
    })?;
    assert!(
        !store
            .connection
            .prepare(include_str!("../sql/deletion/test_foreign_keys.sql"))?
            .exists([])?
    );
    Ok(())
}

#[test]
fn every_foreign_key_has_a_leading_index_for_deletion() -> Result<()> {
    let (_directory, store) = fixture()?;
    let missing = store
        .connection
        .prepare(include_str!(
            "../sql/deletion/test_unindexed_foreign_keys.sql"
        ))?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert!(missing.is_empty(), "Unindexed dependencies: {missing:?}");
    Ok(())
}

#[test]
fn dependent_failure_rolls_back_the_entire_purge_and_search_indexes()
-> Result<()> {
    let (_directory, store) = fixture()?;
    store.connection.execute_batch(include_str!(
        "../sql/deletion/test_reject_cascade.sql"
    ))?;
    assert!(store.purge_deleted(1).is_err());
    assert!(store.is_deleted(1)?);
    assert_eq!(store.comments(1)?.len(), 1);
    store.set_deleted(1, false)?;
    assert_eq!(store.note_versions(1, 1)?.len(), 1);
    assert!(
        !store
            .connection
            .prepare(include_str!("../sql/deletion/test_foreign_keys.sql"))?
            .exists([])?
    );
    Ok(())
}

#[test]
fn empty_or_repeated_cleanup_performs_no_database_writes() -> Result<()> {
    let (_directory, store) = fixture()?;
    store.clean_trash()?;
    let previous = store.connection.total_changes();
    assert_eq!(store.clean_trash()?, 0);
    assert!(!store.purge_deleted(1)?);
    assert_eq!(store.connection.total_changes(), previous);
    Ok(())
}

#[test]
fn image_cleanup_after_purge_keeps_shared_images_and_removes_orphans()
-> Result<()> {
    let (directory, store) = fixture()?;
    let cancel = AtomicBool::new(false);
    store.refresh_image_references(&cancel)?;
    let images = directory.path().join("images");
    let shared = "https://images.example/shared";
    let unique = "https://images.example/unique";
    crate::media::cache::write(&images, shared, b"shared")?;
    crate::media::cache::write(&images, unique, b"unique")?;
    store.purge_deleted(1)?;
    crate::storage::clean(
        &store,
        &images,
        crate::storage::Cleanup::Images,
        &cancel,
    )?;
    assert!(crate::media::cache::path(&images, shared).exists());
    assert!(!crate::media::cache::path(&images, unique).exists());
    Ok(())
}
