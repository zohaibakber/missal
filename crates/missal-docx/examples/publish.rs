use std::path::PathBuf;

use missal_auth::{Config, SessionKeeper};
use missal_docx::{import_folder, repository_templates};
use missal_library::{Library, Upload};
use missal_store::TemplateBundle;

const PREVIEW_LIMIT: usize = 400;

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

fn main() {
    dotenvy::dotenv().ok();
    let mut replace = false;
    let mut source = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--replace" => replace = true,
            _ => source = Some(PathBuf::from(argument)),
        }
    }
    let source = source.unwrap_or_else(repository_templates);
    let folder = import_folder(&source).unwrap_or_else(|error| panic!("{error}"));
    let config = Config::resolve().expect("set API_URL to the Missal API");
    let keeper = SessionKeeper::new(&config, None);
    keeper
        .sign_in(&required("PUBLISH_EMAIL"), &required("PUBLISH_PASSWORD"))
        .unwrap_or_else(|error| panic!("sign-in failed: {error}"));
    let token = || {
        keeper
            .access_token()
            .unwrap_or_else(|error| panic!("session ended: {error}"))
    };
    let library = Library::new(&config.base_url);
    let catalog = library
        .catalog(&token())
        .unwrap_or_else(|error| panic!("could not list the library: {error}"));
    assert!(
        catalog.publisher,
        "this account cannot publish; make it a publisher or admin"
    );
    let (mut added, mut updated, mut skipped) = (0, 0, 0);
    for template in folder.templates {
        for notice in &template.notices {
            eprintln!("{}: {notice}", template.name);
        }
        let existing = catalog
            .templates
            .iter()
            .find(|entry| entry.name == template.name)
            .map(|entry| entry.id.as_str());
        if existing.is_some() && !replace {
            skipped += 1;
            continue;
        }
        let projections = template.document.projections();
        let upload = Upload {
            name: template.name.clone(),
            field_count: projections.field_count as i64,
            preview: projections
                .preview_text
                .chars()
                .take(PREVIEW_LIMIT)
                .collect(),
            content: TemplateBundle::encode(
                1,
                &template.global,
                &template.custom,
                &[(template.name.clone(), template.document)],
            ),
        };
        let published = library
            .publish(&token(), existing, &upload)
            .unwrap_or_else(|error| panic!("{}: {error}", template.name));
        if existing.is_some() {
            updated += 1;
        } else {
            added += 1;
        }
        println!("{} v{}: {}", published.id, published.version, template.name);
    }
    println!("{added} added, {updated} updated, {skipped} already in the library");
}
