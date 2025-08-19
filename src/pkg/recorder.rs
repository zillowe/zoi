use crate::pkg::types::{Package, Scope};
use cyclonedx_bom::{
    external_models::spdx::SpdxExpression,
    models::{
        component::{Classification, Component, Components},
        license::{LicenseChoice, Licenses},
        metadata::Metadata,
        property::{Properties, Property},
        tool::{Tool, Tools},
    },
    prelude::*,
};
use purl::GenericPurl;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

const ZOI_REPO_PROPERTY: &str = "zoi:repo";
const ZOI_CHOSEN_OPTIONS_PROPERTY: &str = "zoi:chosen_options";
const ZOI_CHOSEN_OPTIONALS_PROPERTY: &str = "zoi:chosen_optionals";
const ZOI_SCOPE_PROPERTY: &str = "zoi:scope";

#[derive(Clone)]
pub struct Supplier {
    pub name: String,
    pub url: Option<String>,
}

impl Supplier {
    pub fn new(name: &str) -> Self {
        Supplier {
            name: name.to_string(),
            url: None,
        }
    }
}

#[derive(Clone)]
pub struct Uri(pub String);

impl Uri {
    pub fn new(uri: &str) -> Self {
        Uri(uri.to_string())
    }
}

fn get_sbom_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let path = home_dir.join(".zoi").join("pkgs").join("zoi.pkgs.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn read_sbom() -> Result<Bom, Box<dyn Error>> {
    let path = get_sbom_path()?;
    if !path.exists() || fs::read_to_string(&path)?.trim().is_empty() {
        return Ok(Bom {
            metadata: Some(Metadata {
                tools: Some(Tools::List(vec![Tool {
                    vendor: Some(NormalizedString::new("zillowe")),
                    name: Some(NormalizedString::new("zoi")),
                    version: Some(NormalizedString::new(env!("CARGO_PKG_VERSION"))),
                    ..Tool::default()
                }])),
                ..Metadata::default()
            }),
            ..Bom::default()
        });
    }
    let content = fs::read_to_string(path)?;
    let bom = Bom::parse_from_json(content.as_bytes())?;
    Ok(bom)
}

fn write_sbom(bom: &Bom) -> Result<(), Box<dyn Error>> {
    let path = get_sbom_path()?;
    let mut output = Vec::<u8>::new();
    bom.clone().output_as_json_v1_5(&mut output)?;
    fs::write(path, output)?;
    Ok(())
}

fn create_purl(pkg: &Package) -> Option<String> {
    let version = pkg.version.as_ref()?;
    Some(format!("pkg:zoi/{}/{}@{}", pkg.repo, pkg.name, version))
}

pub fn record_package(
    pkg: &Package,
    chosen_options: &[String],
    chosen_optionals: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut bom = read_sbom()?;
    let version = pkg.version.as_ref().ok_or("Package version not resolved")?;
    let purl_str = create_purl(pkg);
    let bom_ref = purl_str.clone().unwrap_or_else(|| pkg.name.clone());
    let component_type = match pkg.package_type {
        crate::pkg::types::PackageType::Library => Classification::Library,
        _ => Classification::Application,
    };
    let licenses = if !pkg.license.is_empty() {
        Some(Licenses(vec![LicenseChoice::Expression(
            SpdxExpression::new(&pkg.license),
        )]))
    } else {
        None
    };
    let supplier = if let Some(a) = &pkg.author {
        let mut supplier = Supplier::new(&a.name);
        if let Some(website) = &a.website {
            supplier.url = Some(website.clone());
        }
        Some(supplier)
    } else {
        None
    };
    let mut properties = vec![Property::new(ZOI_REPO_PROPERTY, &pkg.repo)];

    if let Some(s) = supplier {
        properties.push(Property::new("zoi:supplier:name", &s.name));
        if let Some(url) = s.url {
            properties.push(Property::new("zoi:supplier:url", &url));
        }
    }

    let scope_str = match pkg.scope {
        Scope::User => "user",
        Scope::System => "system",
    };
    properties.push(Property::new(ZOI_SCOPE_PROPERTY, scope_str));

    if !chosen_options.is_empty() {
        properties.push(Property::new(
            ZOI_CHOSEN_OPTIONS_PROPERTY,
            &chosen_options.join(","),
        ));
    }
    if !chosen_optionals.is_empty() {
        properties.push(Property::new(
            ZOI_CHOSEN_OPTIONALS_PROPERTY,
            &chosen_optionals.join(","),
        ));
    }

    let mut component = Component::new(component_type, &pkg.name, version, Some(bom_ref.clone()));

    if let Some(purl_str_val) = purl_str
        && let Ok(purl) = GenericPurl::<String>::from_str(&purl_str_val)
    {
        properties.push(Property::new("purl", &purl.to_string()));
    }

    component.licenses = licenses;
    component.properties = Some(Properties(properties));

    if let Some(components) = bom.components.as_mut() {
        components
            .0
            .retain(|c| c.name != NormalizedString::new(&pkg.name));
        components.0.push(component);
        components.0.sort_by(|a, b| a.name.cmp(&b.name));
    } else {
        bom.components = Some(Components(vec![component]));
    }
    bom.version += 1;
    write_sbom(&bom)
}

pub fn remove_package_from_record(package_name: &str) -> Result<(), Box<dyn Error>> {
    let mut bom = read_sbom()?;
    let initial_len = bom.components.as_ref().map_or(0, |c| c.0.len());
    if let Some(components) = bom.components.as_mut() {
        components
            .0
            .retain(|c| c.name != NormalizedString::new(package_name));
    }
    let new_len = bom.components.as_ref().map_or(0, |c| c.0.len());
    if new_len < initial_len {
        bom.version += 1;
        write_sbom(&bom)?
    }
    Ok(())
}
