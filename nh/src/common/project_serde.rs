
use std::collections::{HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::{any::Any, collections::HashMap, path::PathBuf};

use crate::common::uuid::ControllerUuid;
use crate::egui;
use serde::{Deserialize, Serialize};

use crate::common::controller::{ColorBundle, DiagramView, HierarchyNode};
use crate::DDes;

use super::entity::EntityUuid;
use super::eref::ERef;
use super::{controller::DiagramController, ufoption::UFOption, uuid::{ModelUuid, ViewUuid}};

pub fn no_dependencies<T>(_t: &T) -> Vec<EntityUuid> {
    vec![]
}

pub trait FSWriteAbstraction {
    fn write_manifest_file(&mut self, bytes: &[u8]) -> Result<(), std::io::Error>;
    fn write_source_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error>;
}

pub struct FSRawWriter<'a> {
    root: &'a Path,
    project_file_name: &'a OsStr,
    sources_folder: &'a OsStr,
}

impl<'a> FSRawWriter<'a> {
    pub fn new(
        root: &'a Path,
        project_file_name: &'a OsStr,
        sources_folder: &'a OsStr,
    ) -> Result<Self, std::io::Error> {
        std::fs::DirBuilder::new().recursive(true).create(root.join(sources_folder).join("documents"))?;
        std::fs::DirBuilder::new().recursive(true).create(root.join(sources_folder).join("models"))?;
        std::fs::DirBuilder::new().recursive(true).create(root.join(sources_folder).join("views"))?;
        std::fs::DirBuilder::new().recursive(true).create(root.join(sources_folder).join("controllers"))?;

        Ok(Self {
            root,
            project_file_name,
            sources_folder,
        })
    }
}

impl FSWriteAbstraction for FSRawWriter<'_> {
    fn write_manifest_file(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        let path = self.root.join(self.project_file_name);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        file.write_all(bytes)
    }
    fn write_source_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
        let path = self.root.join(self.sources_folder).join(path);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        file.write_all(bytes)
    }
}

pub struct ZipFSWriter<'a> {
    zip: zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
    project_file_name: &'a str,
    sources_folder: &'a str,
}

impl<'a> ZipFSWriter<'a> {
    pub fn new(
        project_file_name: &'a str,
        sources_folder: &'a str,
    ) -> Self {
        Self {
            zip: zip::ZipWriter::new(std::io::Cursor::new(Vec::new())),
            project_file_name,
            sources_folder,
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>, std::io::Error> {
        Ok(self.zip.finish()?.into_inner())
    }
}

impl FSWriteAbstraction for ZipFSWriter<'_> {
    fn write_manifest_file(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        self.zip.start_file(self.project_file_name, zip::write::SimpleFileOptions::default())?;
        self.zip.write_all(bytes)
    }
    fn write_source_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
        let path = format!("{}/{}", self.sources_folder, path);
        self.zip.start_file(path, zip::write::SimpleFileOptions::default())?;
        self.zip.write_all(bytes)
    }
}


pub trait FSReadAbstraction {
    fn read_manifest_file(&mut self) -> Result<Vec<u8>, std::io::Error>;
    fn set_source_folder(&mut self, sources_folder: &str);
    fn read_source_file(&mut self, path: &str) -> Result<Vec<u8>, std::io::Error>;
}


pub struct FSRawReader {
    root: PathBuf,
    project_file_name: OsString,
    sources_folder: String,
}

impl<'a> FSRawReader {
    pub fn new(
        root: PathBuf,
        project_file_name: OsString,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            root,
            project_file_name,
            sources_folder: ".".to_owned(),
        })
    }
}

impl FSReadAbstraction for FSRawReader {
    fn read_manifest_file(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let path = self.root.join(&self.project_file_name);
        let mut file = std::fs::File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }
    fn set_source_folder(&mut self, sources_folder: &str) {
        self.sources_folder = sources_folder.to_owned();
    }
    fn read_source_file(&mut self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        let path = self.root.join(&self.sources_folder).join(path);
        let mut file = std::fs::File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }
}

pub struct ZipFSReader<'a> {
    zip: zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    project_file_name: &'a str,
    sources_folder: &'a str,
}

impl<'a> ZipFSReader<'a> {
    pub fn new(
        bytes: Vec<u8>,
        project_file_name: &'a str,
        sources_folder: &'a str,
    ) -> Result<Self, std::io::Error> {
        let zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;

        Ok(Self {
            zip,
            project_file_name,
            sources_folder,
        })
    }
}

impl FSReadAbstraction for ZipFSReader<'_> {
    fn read_manifest_file(&mut self) -> Result<Vec<u8>, std::io::Error> {
        self.zip.by_name(self.project_file_name)?.bytes().collect()
    }
    fn set_source_folder(&mut self, _sources_folder: &str) {}
    fn read_source_file(&mut self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        let path = format!("{}/{}", self.sources_folder, path);
        self.zip.by_name(&path)?.bytes().collect()
    }
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum NHProjectHierarchyNodeSerialization {
    Folder { uuid: ViewUuid, name: String, hierarchy: Vec<NHProjectHierarchyNodeSerialization> },
    Diagram { uuid: ViewUuid },
    Document { uuid: ViewUuid, name: String, },
}

#[derive(Serialize, Deserialize, Debug)]
struct GlobalColorDTO {
    uuid: uuid::Uuid,
    name: String,
    color: egui::Color32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHProjectSerialization {
    format_version: String,
    project_name: String,
    sources_root: String,
    new_diagram_no_counter: usize,
    hierarchy: Vec<NHProjectHierarchyNodeSerialization>,
    controllers: Vec<NHControllerInfo>,
    global_colors: Vec<GlobalColorDTO>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHControllerInfo {
    uuid: ControllerUuid,
    controller_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHEntitySerialization {
    depends_on: Vec<EntityUuid>,
    main: toml::Value,
    other: Vec<toml::Value>,
}

impl NHProjectSerialization {
    pub fn write_to<WA: FSWriteAbstraction>(
        wa: &mut WA,
        project_name: &str,
        sources_root: &str,
        new_diagram_no_counter: usize,
        hierarchy: &Vec<HierarchyNode>,
        global_colors: &ColorBundle,
        diagram_controllers: &HashMap<ViewUuid, (usize, ERef<dyn DiagramController>)>,
        documents: &HashMap<ViewUuid, (String, String)>,
    ) -> Result<(), NHSerializeError> {
        fn h(e: &HierarchyNode, d: &HashMap<ViewUuid, (String, String)>) -> NHProjectHierarchyNodeSerialization {
            match e {
                HierarchyNode::Folder(uuid, name, children)
                    => NHProjectHierarchyNodeSerialization::Folder {
                        uuid: *uuid,
                        name: (**name).clone(),
                        hierarchy: children.iter().map(|e| h(e, d)).collect(),
                    },
                HierarchyNode::Diagram(inner) => NHProjectHierarchyNodeSerialization::Diagram { uuid: *inner.read().uuid() },
                HierarchyNode::Document(uuid) => {
                    NHProjectHierarchyNodeSerialization::Document { uuid: *uuid, name: d.get(uuid).unwrap().0.clone() }
                }
            }
        }

        let mut serializer = NHSerializer::new();
        let mut unique_diagram_controllers = HashMap::new();
        for e in diagram_controllers.iter() {
            let r = e.1.1.read();
            r.serialize_into(&mut serializer)?;
            unique_diagram_controllers.insert(*r.uuid(), r.controller_type().to_owned());
        }
        NHSerializer::write_all(serializer, wa)?;

        for (key, (_, content)) in documents.iter() {
            wa.write_source_file(&format!("documents/{}.nhd", key.to_string()), content.as_bytes())?;
        }

        let global_colors = global_colors.colors_order.iter()
            .flat_map(|k| global_colors.colors.get(k).map(|e| GlobalColorDTO {
                uuid: *k, name: e.0.clone(), color: e.1,
            })).collect();

        let project_serialization = Self {
            format_version: env!("COMMIT_IDENTIFIER").to_owned(),
            project_name: project_name.to_owned(),
            sources_root: sources_root.to_owned(),
            new_diagram_no_counter,
            hierarchy: hierarchy.iter().map(|e| h(e, documents)).collect(),
            controllers: {
                let mut controllers: Vec<_> = unique_diagram_controllers
                    .into_iter()
                    .map(|e| NHControllerInfo { uuid: e.0, controller_type: e.1 })
                    .collect();
                controllers.sort_by_key(|e| e.uuid);
                controllers
            },
            global_colors,
        };
        wa.write_manifest_file(toml::to_string(&project_serialization)?.as_bytes())?;

        Ok(())
    }

    pub fn project_name(&self) -> String {
        self.project_name.clone()
    }
    pub fn new_diagram_no_counter(&self) -> usize {
        self.new_diagram_no_counter
    }
    pub fn global_colors(&self) -> ColorBundle {
        let o = self.global_colors.iter().map(|e| e.uuid).collect();
        let c = self.global_colors.iter().map(|e| (e.uuid, (e.name.clone(), e.color))).collect();
        ColorBundle { colors_order: o, colors: c }
    }

    pub fn deserialize_all(
        &self,
        ra: &mut dyn FSReadAbstraction,
        diagram_deserializers: &HashMap<String, (usize, &'static DDes)>,
    ) -> Result<(
            Vec<HierarchyNode>,
            HashMap<ViewUuid, (usize, ERef<dyn DiagramController>)>,
            HashMap<ViewUuid, (String, String)>,
        ),
        NHDeserializeError
    > {
        ra.set_source_folder(&self.sources_root);
        let mut deserializer = NHDeserializer::new(ra);

        // Load all necessary sources
        fn l(e: &NHProjectHierarchyNodeSerialization, d: &mut NHDeserializer) -> Result<(), NHDeserializeError> {
            match e {
                NHProjectHierarchyNodeSerialization::Folder { hierarchy, .. } => {
                    for e in hierarchy {
                        l(e, d)?;
                    }
                    Ok(())
                },
                NHProjectHierarchyNodeSerialization::Diagram { uuid, .. } => {
                    Ok(d.load_sources(EntityUuid::View(*uuid))?)
                },
                NHProjectHierarchyNodeSerialization::Document { .. } => {
                    Ok(())
                }
            }
        }

        for e in &self.hierarchy {
            l(e, &mut deserializer)?;
        }
        for e in &self.controllers {
            deserializer.load_sources((*e).uuid.into())?;
        }

        // Instantiate all entities
        let mut top_level_controllers = HashMap::new();
        let mut top_level_views = HashMap::new();
        for e in &self.controllers {
            let (type_no, dd) = diagram_deserializers.get(&e.controller_type)
                .ok_or_else(|| format!("deserializer for type '{}' not found", e.controller_type))?;
            let controller = dd(e.uuid, &mut deserializer)?;
            let r = controller.read();
            for e in r.view_uuids() {
                top_level_controllers.insert(e, (*type_no, controller.clone()));
                top_level_views.insert(e, r.get(&e).unwrap());
            }
        }

        fn h(
            e: &NHProjectHierarchyNodeSerialization,
            d: &mut NHDeserializer,
            views: &HashMap<ViewUuid, ERef<dyn DiagramView>>,
            docs: &mut HashMap<ViewUuid, (String, String)>,
            dds: &HashMap<String, (usize, &'static DDes)>,
        ) -> Result<HierarchyNode, NHDeserializeError> {
            match e {
                NHProjectHierarchyNodeSerialization::Folder { uuid, name, hierarchy }
                => Ok(HierarchyNode::Folder(
                        *uuid, Arc::new((*name).clone()),
                        hierarchy.iter().map(|e| h(e, d, views, docs, dds)).collect::<Result<Vec<_>, NHDeserializeError>>()?,
                    )),
                NHProjectHierarchyNodeSerialization::Diagram { uuid }
                => {
                    let view = views.get(uuid).ok_or_else(|| format!("view '{:?}' not found", uuid))?;
                    Ok(HierarchyNode::Diagram(view.clone()))
                }
                NHProjectHierarchyNodeSerialization::Document { uuid, name }
                => {
                    let path = format!("documents/{}.nhd", uuid.to_string());
                    let bytes = d.ra.read_source_file(&path)?;
                    let content = str::from_utf8(&bytes)?.to_owned();
                    docs.insert(*uuid, (name.clone(), content));
                    Ok(HierarchyNode::Document(*uuid))
                }
            }
        }

        let mut hierarchy = Vec::new();
        let mut documents = HashMap::new();

        for e in &self.hierarchy {
            hierarchy.push(h(e, &mut deserializer, &top_level_views, &mut documents, diagram_deserializers)?);
        }

        Ok((hierarchy, top_level_controllers, documents))
    }
}

pub struct NHSerializer {
    stack: Vec<(EntityUuid, Vec<EntityUuid>, HashMap<EntityUuid, toml::Table>)>,
    all_contained: HashSet<EntityUuid>,
    closed_subsets: HashMap<EntityUuid, (Vec<EntityUuid>, HashMap<EntityUuid, toml::Table>)>,
}

impl NHSerializer {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            all_contained: HashSet::new(),
            closed_subsets: HashMap::new(),
        }
    }

    fn write_all<WA: FSWriteAbstraction>(self, wa: &mut WA) -> Result<(), NHSerializeError> {
        for e in self.closed_subsets {
            let (filename, subset) = match e {
                (u @ EntityUuid::Model(model_uuid), (depends_on, mut e)) => {
                    let main = toml::Value::Table(e.remove(&u).unwrap());
                    let mut models: Vec<_> = e.into_iter().collect();
                    models.sort_by_key(|e| e.0);
                    let subset = NHEntitySerialization {
                        depends_on,
                        main,
                        other: models.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
                    };
                    (
                        format!("models/{}.nhe", model_uuid.to_string()),
                        toml::to_string(&subset)?,
                    )
                },
                (u @ EntityUuid::View(view_uuid), (depends_on, mut e)) => {
                    let main = toml::Value::Table(e.remove(&u).unwrap());
                    let mut views: Vec<_> = e.into_iter().collect();
                    views.sort_by_key(|e| e.0);
                    let subset = NHEntitySerialization {
                        depends_on,
                        main,
                        other: views.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
                    };
                    (
                        format!("views/{}.nhe", view_uuid.to_string()),
                        toml::to_string(&subset)?,
                    )
                },
                (u @ EntityUuid::Controller(controller_uuid), (depends_on, mut e)) => {
                    let main = toml::Value::Table(e.remove(&u).unwrap());
                    let mut controllers: Vec<_> = e.into_iter().collect();
                    controllers.sort_by_key(|e| e.0);
                    let subset = NHEntitySerialization {
                        depends_on,
                        main,
                        other: controllers.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
                    };
                    (
                        format!("controllers/{}.nhe", controller_uuid.to_string()),
                        toml::to_string(&subset)?,
                    )
                },
            };

            wa.write_source_file(&filename, subset.as_bytes())?;
        }

        Ok(())
    }
}

impl NHSerializer {
    pub fn contains(&self, uuid: &EntityUuid) -> bool {
        self.all_contained.contains(uuid)
    }

    pub fn open_new_subset(&mut self, uuid: EntityUuid, depends_on: Vec<EntityUuid>) {
        self.stack.push((uuid, depends_on, HashMap::new()));
    }
    pub fn close_last_subset(&mut self) {
        let (uuid, depends_on, subset) = self.stack.pop().unwrap();
        self.closed_subsets.insert(uuid, (depends_on, subset));
    }

    pub fn insert(&mut self, uuid: EntityUuid, data: toml::Table) {
        self.all_contained.insert(uuid);
        self.stack.last_mut().unwrap().2.insert(uuid, data);
    }
}

#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
pub enum NHSerializeError {
    StructureError(String),
    TomlSer(toml::ser::Error),
    IoError(std::io::Error),
}

pub trait NHContextSerialize {
    #[clippy::must_use]
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError>;
}

impl<T> NHContextSerialize for UFOption<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        match self {
            UFOption::None => Ok(()),
            UFOption::Some(e) => e.serialize_into(into),
        }
    }
}

impl<T> NHContextSerialize for Vec<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        for e in self.iter() {
            e.serialize_into(into)?;
        }
        Ok(())
    }
}

impl<T> NHContextSerialize for ERef<T> where T: NHContextSerialize {
    fn serialize_into(&self, into: &mut NHSerializer) -> Result<(), NHSerializeError> {
        self.read().serialize_into(into)
    }
}

pub struct NHDeserializer<'a> {
    ra: &'a mut dyn FSReadAbstraction,
    // TODO: this could have been two HashMaps total
    source_models: HashMap<ModelUuid, toml::Table>,
    source_views: HashMap<ViewUuid, toml::Table>,
    source_controllers: HashMap<ControllerUuid, toml::Table>,
    instantiated_models: HashMap<ModelUuid, Box<dyn Any>>,
    instantiated_views: HashMap<ViewUuid, Box<dyn Any>>,
    instantiated_controllers: HashMap<ControllerUuid, Box<dyn Any>>,
}

impl<'a> NHDeserializer<'a> {
    fn new(ra: &'a mut dyn FSReadAbstraction) -> Self {
        Self {
            ra,
            source_models: HashMap::new(),
            source_views: HashMap::new(),
            source_controllers: HashMap::new(),
            instantiated_models: HashMap::new(),
            instantiated_views: HashMap::new(),
            instantiated_controllers: HashMap::new(),
        }
    }
    fn load_sources(&mut self, uuid: EntityUuid) -> Result<(), NHDeserializeError> {
        let mut queue: VecDeque<_> = std::iter::once(uuid).collect();
        // TODO: cycle detection?
        while let Some(uuid) = queue.pop_front() {
            match uuid {
                EntityUuid::Model(model_uuid) => {
                    let path = format!("models/{}.nhe", model_uuid.to_string());
                    let bytes = self.ra.read_source_file(&path)?;
                    let content = str::from_utf8(&bytes)?;
                    let NHEntitySerialization { depends_on, main, other } = toml::from_str(&content)?;

                    queue.extend(depends_on);
                    let toml::Value::Table(main_model) = main else {
                        return Err(format!("expected table, found {:?}", main).into());
                    };
                    self.source_models.insert(get_model_uuid(&main_model)?, main_model);
                    for v in other {
                        let toml::Value::Table(t) = v else {
                            return Err(format!("expected table, found {:?}", v).into());
                        };
                        self.source_models.insert(get_model_uuid(&t)?, t);
                    }
                },
                EntityUuid::View(view_uuid) => {
                    let path = format!("views/{}.nhe", view_uuid.to_string());
                    let bytes = self.ra.read_source_file(&path)?;
                    let content = str::from_utf8(&bytes)?;
                    let NHEntitySerialization { depends_on, main, other } = toml::from_str(&content)?;

                    queue.extend(depends_on);
                    let toml::Value::Table(main_view) = main else {
                        return Err(format!("expected table, found {:?}", main).into());
                    };
                    self.source_views.insert(get_view_uuid(&main_view)?, main_view);
                    for v in other {
                        let toml::Value::Table(t) = v else {
                            return Err(format!("expected table, found {:?}", v).into());
                        };
                        self.source_views.insert(get_view_uuid(&t)?, t);
                    }
                },
                EntityUuid::Controller(controller_uuid) => {
                    let path = format!("controllers/{}.nhe", controller_uuid.to_string());
                    let bytes = self.ra.read_source_file(&path)?;
                    let content = str::from_utf8(&bytes)?;
                    let NHEntitySerialization { depends_on, main, other } = toml::from_str(&content)?;

                    queue.extend(depends_on);
                    let toml::Value::Table(main_controller) = main else {
                        return Err(format!("expected table, found {:?}", main).into());
                    };
                    self.source_controllers.insert(get_controller_uuid(&main_controller)?, main_controller);
                    for v in other {
                        let toml::Value::Table(t) = v else {
                            return Err(format!("expected table, found {:?}", v).into());
                        };
                        self.source_controllers.insert(get_controller_uuid(&t)?, t);
                    }
                },
            }
        }
        Ok(())
    }
}

pub trait NHDeserializeInstantiator<K> {
    fn get_entity<T>(&mut self, uuid: &K) -> Result<ERef<T>, NHDeserializeError>
    where T: NHContextDeserialize + 'static;
}

macro_rules! deserialize_instantiator {
    ($uuid_type:ty, $instantiated:ident, $source:ident) => {
        impl NHDeserializeInstantiator<$uuid_type> for NHDeserializer<'_> {
            fn get_entity<T>(&mut self, uuid: &$uuid_type) -> Result<ERef<T>, NHDeserializeError>
            where T: NHContextDeserialize + 'static,
            {
                if let Some(e) = self.$instantiated.get(uuid) {
                    return Ok(e.downcast_ref::<ERef<T>>()
                        .ok_or(NHDeserializeError::StructureError(format!("element has unexpected type: {:?}", uuid)))?
                        .clone());
                }

                let Some(e) = self.$source.get(uuid).cloned().map(|e| toml::Value::Table(e)) else {
                    return Err(NHDeserializeError::StructureError(format!("element not found in source: {:?}", uuid)));
                };

                let e = ERef::new(T::deserialize(&e, self)?);
                self.$instantiated.insert(*uuid, Box::new(e.clone()));
                Ok(e)
            }
        }
    };
}

deserialize_instantiator!(ModelUuid, instantiated_models, source_models);
deserialize_instantiator!(ViewUuid, instantiated_views, source_views);
deserialize_instantiator!(ControllerUuid, instantiated_controllers, source_controllers);

#[expect(dead_code)]
#[derive(Debug, derive_more::From)]
pub enum NHDeserializeError {
    StructureError(String),
    UuidError(uuid::Error),
    TomlError(toml::de::Error),
    IoError(std::io::Error),
    Utf8Error(std::str::Utf8Error),
}

pub trait NHContextDeserialize: Sized {
    fn deserialize(
        source: &toml::Value,
        deserializer: &mut NHDeserializer,
    ) -> Result<Self, NHDeserializeError>;
}

impl<T> NHContextDeserialize for UFOption<T> where T: NHContextDeserialize {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        #[derive(serde::Deserialize)]
        enum Helper {
            None,
            Some(toml::Value),
        }
        match toml::Value::try_into(source.clone())? {
            Helper::None => Ok(UFOption::None),
            Helper::Some(v) => Ok(UFOption::Some(T::deserialize(&v, deserializer)?)),
        }
    }
}
impl<T> NHContextDeserialize for Vec<T> where T: NHContextDeserialize {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        source.as_array().ok_or_else(|| NHDeserializeError::StructureError("expected array".into()))?
            .iter().map(|e| T::deserialize(e, deserializer)).collect()
    }
}
impl<T> NHContextDeserialize for ERef<T> where T: NHContextDeserialize + 'static {
    fn deserialize(source: &toml::Value, deserializer: &mut NHDeserializer) -> Result<Self, NHDeserializeError> {
        let uuid = toml::Value::try_into(source.clone())?;
        match uuid {
            EntityUuid::Model(uuid) => deserializer.get_entity(&uuid),
            EntityUuid::View(uuid) => deserializer.get_entity(&uuid),
            EntityUuid::Controller(uuid) => deserializer.get_entity(&uuid),
        }
    }
}

pub fn get_model_uuid(table: &toml::Table) -> Result<ModelUuid, NHDeserializeError> {
    let v = table.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing model uuid: {:?}", table)))?;
    let toml::Value::String(s) = v else {
        return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
    };
    Ok(uuid::Uuid::parse_str(s)?.into())
}

pub fn get_view_uuid(table: &toml::Table) -> Result<ViewUuid, NHDeserializeError> {
    let v = table.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing view uuid {:?}", table)))?;
    let toml::Value::String(s) = v else {
        return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
    };
    Ok(uuid::Uuid::parse_str(s)?.into())
}

pub fn get_controller_uuid(table: &toml::Table) -> Result<ControllerUuid, NHDeserializeError> {
    let v = table.get("uuid").ok_or_else(|| NHDeserializeError::StructureError(format!("missing controller uuid {:?}", table)))?;
    let toml::Value::String(s) = v else {
        return Err(NHDeserializeError::StructureError(format!("expected string, found {:?}", v)));
    };
    Ok(uuid::Uuid::parse_str(s)?.into())
}

