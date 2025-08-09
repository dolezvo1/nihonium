
use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::io::Write;
use std::sync::Arc;
use std::{any::Any, collections::HashMap, path::PathBuf, sync::RwLock};

use serde::{Deserialize, Serialize};

use crate::common::controller::HierarchyNode;
use crate::DDes;

use super::entity::EntityUuid;
use super::eref::ERef;
use super::{controller::DiagramController, ufoption::UFOption, uuid::{ModelUuid, ViewUuid}};

pub fn no_dependencies<T>(t: &T) -> Vec<EntityUuid> {
    vec![]
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum NHProjectHierarchyNodeSerialization {
    Folder { uuid: ViewUuid, name: String, hierarchy: Vec<NHProjectHierarchyNodeSerialization> },
    Diagram { uuid: ViewUuid, view_type: String },
    Document { uuid: ViewUuid, name: String, },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHProjectSerialization {
    format_version: String,
    project_name: String,
    sources_root: String,
    new_diagram_no_counter: usize,
    hierarchy: Vec<NHProjectHierarchyNodeSerialization>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHModelSerialization {
    depends_on: Vec<EntityUuid>,
    main_model: toml::Value,
    model: Vec<toml::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NHViewSerialization {
    depends_on: Vec<EntityUuid>,
    main_view: toml::Value,
    view: Vec<toml::Value>,
}

impl NHProjectSerialization {
    pub fn write_to(
        project_file_path: &PathBuf,
        project_name: &str,
        sources_root: &str,
        new_diagram_no_counter: usize,
        hierarchy: &Vec<HierarchyNode>,
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
                HierarchyNode::Diagram(inner)
                    => {
                    let r = inner.read();
                    NHProjectHierarchyNodeSerialization::Diagram { uuid: *r.uuid(), view_type: r.view_type() }
                },
                HierarchyNode::Document(uuid) => {
                    NHProjectHierarchyNodeSerialization::Document { uuid: *uuid, name: d.get(uuid).unwrap().0.clone() }
                }
            }
        }

        let sources_root_abs = project_file_path.parent().map(|e| e.join(sources_root))
            .ok_or_else(|| format!("project_file_path {:?} does not have a valid parent", project_file_path))?;
        let mut serializer = NHSerializer::new();
        for e in diagram_controllers.iter() {
            e.1.1.read().serialize_into(&mut serializer)?;
        }
        NHSerializer::write_all(serializer, &sources_root_abs);

        std::fs::DirBuilder::new().recursive(true).create(sources_root_abs.join("documents"))?;
        for (key, (_, content)) in documents.iter() {
            let path = sources_root_abs.join("documents").join(format!("{}.nhd", key.to_string()));
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path)?;
            file.write_all(content.as_bytes())?;
        }

        let project_serialization = Self {
            format_version: "0.2.0".into(),
            project_name: project_name.to_owned(),
            sources_root: sources_root.to_owned(),
            new_diagram_no_counter,
            hierarchy: hierarchy.iter().map(|e| h(e, documents)).collect(),
        };

        let project_str = toml::to_string(&project_serialization)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&project_file_path)?;
        file.write_all(project_str.as_bytes())?;

        Ok(())
    }

    pub fn project_name(&self) -> String {
        self.project_name.clone()
    }
    pub fn new_diagram_no_counter(&self) -> usize {
        self.new_diagram_no_counter
    }

    pub fn deserialize_all(
        &self,
        project_file_path: &PathBuf,
        diagram_deserializers: &HashMap<String, (usize, &'static DDes)>,
    ) -> Result<(
            Vec<HierarchyNode>,
            HashMap<ViewUuid, (usize, ERef<dyn DiagramController>)>,
            HashMap<ViewUuid, (String, String)>,
        ),
        NHDeserializeError
    > {
        let sources_root = project_file_path.parent().map(|e| e.join(&self.sources_root))
            .ok_or_else(|| format!("project_file_path {:?} does not have a valid parent", project_file_path))?;
        let mut deserializer = NHDeserializer::new(sources_root);

        // Load all necessary sources
        fn l(e: &NHProjectHierarchyNodeSerialization, d: &mut NHDeserializer) -> Result<(), NHDeserializeError> {
            match e {
                NHProjectHierarchyNodeSerialization::Folder { uuid, name, hierarchy } => {
                    for e in hierarchy {
                        l(e, d)?;
                    }
                    Ok(())
                },
                NHProjectHierarchyNodeSerialization::Diagram { uuid, view_type } => {
                    Ok(d.load_sources(EntityUuid::View(*uuid))?)
                },
                NHProjectHierarchyNodeSerialization::Document { uuid, name } => {
                    Ok(())
                }
            }
        }

        for e in &self.hierarchy {
            l(e, &mut deserializer)?;
        }

        // Instantiate all entities
        fn h(
            e: &NHProjectHierarchyNodeSerialization,
            d: &mut NHDeserializer,
            tl: &mut HashMap<ViewUuid, (usize, ERef<dyn DiagramController>)>,
            docs: &mut HashMap<ViewUuid, (String, String)>,
            dds: &HashMap<String, (usize, &'static DDes)>,
        ) -> Result<HierarchyNode, NHDeserializeError> {
            match e {
                NHProjectHierarchyNodeSerialization::Folder { uuid, name, hierarchy }
                => Ok(HierarchyNode::Folder(
                        *uuid, Arc::new((*name).clone()),
                        hierarchy.iter().map(|e| h(e, d, tl, docs, dds)).collect::<Result<Vec<_>, NHDeserializeError>>()?,
                    )),
                NHProjectHierarchyNodeSerialization::Diagram { uuid, view_type }
                => {
                    let (type_no, dd) = dds.get(view_type)
                        .ok_or_else(|| format!("deserializer for type '{}' not found", view_type))?;
                    let view = dd(*uuid, d)?;
                    tl.insert(*uuid, (*type_no, view.clone()));
                    Ok(HierarchyNode::Diagram(view))
                }
                NHProjectHierarchyNodeSerialization::Document { uuid, name }
                => {
                    let path = d.sources_root.join("documents").join(format!("{}.nhd", uuid.to_string()));
                    let content = std::fs::read_to_string(&path)?;
                    docs.insert(*uuid, (name.clone(), content));
                    Ok(HierarchyNode::Document(*uuid))
                }
            }
        }

        let mut hierarchy = Vec::new();
        let mut top_level_views = HashMap::new();
        let mut documents = HashMap::new();

        for e in &self.hierarchy {
            hierarchy.push(h(e, &mut deserializer, &mut top_level_views, &mut documents, diagram_deserializers)?);
        }

        Ok((hierarchy, top_level_views, documents))
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

    fn write_all(self, sources_root: &PathBuf) -> Result<(), NHSerializeError> {
        std::fs::DirBuilder::new().recursive(true).create(sources_root.join("models"))?;
        std::fs::DirBuilder::new().recursive(true).create(sources_root.join("views"))?;

        for e in self.closed_subsets {
            let (path, subset) = match e {
                (u @ EntityUuid::Model(model_uuid), (depends_on, mut e)) => {
                    let main_model = toml::Value::Table(e.remove(&u).unwrap());
                    let mut models: Vec<_> = e.into_iter().collect();
                    models.sort_by_key(|e| e.0);
                    let subset = NHModelSerialization {
                        depends_on,
                        main_model,
                        model: models.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
                    };
                    (
                        sources_root.join("models").join(format!("{}.nhm", model_uuid.to_string())),
                        toml::to_string(&subset)?,
                    )
                },
                (u @ EntityUuid::View(view_uuid), (depends_on, mut e)) => {
                    let main_view = toml::Value::Table(e.remove(&u).unwrap());
                    let mut views: Vec<_> = e.into_iter().collect();
                    views.sort_by_key(|e| e.0);
                    let subset = NHViewSerialization {
                        depends_on,
                        main_view,
                        view: views.into_iter().map(|e| toml::Value::Table(e.1)).collect(),
                    };
                    (
                        sources_root.join("views").join(format!("{}.nhv", view_uuid.to_string())),
                        toml::to_string(&subset)?,
                    )
                },
            };

            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path)?;
            file.write_all(subset.as_bytes())?;
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

pub struct NHDeserializer {
    sources_root: PathBuf,
    source_models: HashMap<ModelUuid, toml::Table>,
    source_views: HashMap<ViewUuid, toml::Table>,
    instantiated_models: HashMap<ModelUuid, Box<dyn Any>>,
    instantiated_views: HashMap<ViewUuid, Box<dyn Any>>,
}

impl NHDeserializer {
    fn new(sources_root: PathBuf) -> Self {
        Self {
            sources_root,
            source_models: HashMap::new(),
            source_views: HashMap::new(),
            instantiated_models: HashMap::new(),
            instantiated_views: HashMap::new(),
        }
    }
    fn load_sources(&mut self, uuid: EntityUuid) -> Result<(), NHDeserializeError> {
        let mut queue: VecDeque<_> = std::iter::once(uuid).collect();
        // TODO: cycle detection?
        while let Some(uuid) = queue.pop_front() {
            match uuid {
                EntityUuid::Model(model_uuid) => {
                    let path = self.sources_root.join("models").join(format!("{}.nhm", model_uuid.to_string()));
                    let content = std::fs::read_to_string(&path)?;
                    let NHModelSerialization { depends_on, main_model, model } = toml::from_str(&content)?;

                    queue.extend(depends_on);
                    let toml::Value::Table(main_model) = main_model else {
                        return Err(format!("expected table, found {:?}", main_model).into());
                    };
                    self.source_models.insert(get_model_uuid(&main_model)?, main_model);
                    for v in model {
                        let toml::Value::Table(t) = v else {
                            return Err(format!("expected table, found {:?}", v).into());
                        };
                        self.source_models.insert(get_model_uuid(&t)?, t);
                    }
                },
                EntityUuid::View(view_uuid) => {
                    let path = self.sources_root.join("views").join(format!("{}.nhv", view_uuid.to_string()));
                    let content = std::fs::read_to_string(&path)?;
                    let NHViewSerialization { depends_on, main_view, view } = toml::from_str(&content)?;

                    queue.extend(depends_on);
                    let toml::Value::Table(main_view) = main_view else {
                        return Err(format!("expected table, found {:?}", main_view).into());
                    };
                    self.source_views.insert(get_view_uuid(&main_view)?, main_view);
                    for v in view {
                        let toml::Value::Table(t) = v else {
                            return Err(format!("expected table, found {:?}", v).into());
                        };
                        self.source_views.insert(get_view_uuid(&t)?, t);
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
        impl NHDeserializeInstantiator<$uuid_type> for NHDeserializer {
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

#[derive(Debug, derive_more::From)]
pub enum NHDeserializeError {
    StructureError(String),
    UuidError(uuid::Error),
    TomlError(toml::de::Error),
    IoError(std::io::Error),
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

