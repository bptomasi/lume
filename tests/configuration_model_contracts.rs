//! Portable configuration round-trips domain import bounds independently.

#[test]
fn limits_round_trip_and_reject_unbounded_refreshes() -> anyhow::Result<()> {
    let config = lume::config::decode(
        "[[servers]]\n\
         id='home'\n\
         url='https://reader.test'\n\
         token_env='TOKEN'\n\
         [servers.sync]\n\
         initial_entries=0\n\
         entries_per_page=25\n\
         pages_per_sync=1",
    )?;
    assert_eq!(config.servers[0].sync.initial_entries, 0);
    assert_eq!(
        lume::config::decode(&lume::config::encode(&config)?)?,
        config
    );
    for (field, value) in [
        ("initial_entries", 10_000_001),
        ("entries_per_page", 0),
        ("entries_per_page", 201),
        ("pages_per_sync", 0),
        ("pages_per_sync", 11),
    ] {
        let mut value_json =
            serde_json::to_value(lume::model::ServerSync::default())?;
        value_json[field] = value.into();
        assert!(
            serde_json::from_value::<lume::model::ServerSync>(value_json)?
                .validate()
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn standalone_bindings_preserve_aliases_and_unicode_keys() -> anyhow::Result<()>
{
    let text = "[ui.keybindings]\n'alt-λ' = 'fs'\n\
                'ctrl-f12' = 'workspace save Étude'\n";
    let config = lume::config::decode(text)?;
    assert_eq!(config.ui.keybindings["alt-λ"], "fs");
    assert_eq!(config.ui.keybindings["ctrl-f12"], "workspace save Étude");
    assert_eq!(
        lume::config::decode(&lume::config::encode(&config)?)?,
        config
    );
    for binding in [
        "'ctrl-c' = 'fs'",
        "'f13' = 'fullscreen'",
        "'ctrl-t' = 'unregistered-command'",
    ] {
        assert!(
            lume::config::decode(&format!("[ui.keybindings]\n{binding}\n"))
                .is_err()
        );
    }
    Ok(())
}
