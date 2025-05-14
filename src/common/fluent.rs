
use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::{langid, LanguageIdentifier};

pub const AVAILABLE_LANGUAGES: &[(LanguageIdentifier, &str)] = &[
    (langid!("en-US"), include_str!("localization/en.ftl")),
    (langid!("cs-CZ"), include_str!("localization/cs.ftl")),
];


pub fn create_fluent_bundle(desired_languages: &Vec<LanguageIdentifier>) -> Result<FluentBundle<FluentResource>, String> {

    let mut bundle = FluentBundle::new(desired_languages.clone());

    for l in desired_languages.iter().rev() {
        let Some((_, s)) = AVAILABLE_LANGUAGES.iter().find(|e| e.0 == *l) else {
            return Err(format!("Language {} not supported", l));
        };
        let resource = FluentResource::try_new((*s).to_owned())
            .map_err(|e| format!("Parsing language {} failed: {:?}", l, e))?;
        bundle.add_resource_overriding(resource);
    }

    Ok(bundle)
}
