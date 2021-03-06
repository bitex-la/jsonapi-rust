pub use std::collections::HashMap;
pub use api::*;
pub use query::{Query, QueryFields};
use errors::*;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, to_value, Value, Map};

/// A trait for any struct that can be converted from/into a Resource.
/// The only requirement is that your struct has an 'id: String' field.
/// You shouldn't be implementing JsonApiModel manually, look at the
/// `jsonapi_model!` macro instead.
pub trait JsonApiModel: Serialize
    where for<'de> Self: Deserialize<'de>
{
    #[doc(hidden)]
    fn jsonapi_type() -> &'static str;
    #[doc(hidden)]
    fn jsonapi_id(&self) -> Option<String>;
    #[doc(hidden)]
    fn relationship_fields() -> Option<&'static [&'static str]>;
    #[doc(hidden)]
    fn build_relationships(&self, query: &QueryFields) -> Option<Relationships>;
    #[doc(hidden)]
    fn build_included(&self, fields: &Option<Vec<String>>) -> Option<Resources>;

    fn should_serialize_field(&self, query: &QueryFields, field: &str) -> bool {
      if query.is_none(){ return true }
      let hashmap = query.as_ref().unwrap();
      let fields = hashmap.get(&Self::jsonapi_type().to_string());
      if fields.is_none(){ return true }
      fields.unwrap().contains(&field.to_string())
    }

    fn should_include(&self, included: &Option<Vec<String>>, field: &str) -> bool {
        if included.is_none() { return true }
        included.as_ref().unwrap().contains(&field.to_string())
    }

    fn from_jsonapi_resource(resource: &Resource, included: &Option<Resources>)
        -> Result<Self> 
    {
        Self::from_serializable(Self::resource_to_attrs(resource, included))
    }

    fn from_jsonapi_document(doc: &JsonApiDocument) -> Result<Self> {
        match doc.data.as_ref() {
            Some(primary_data) => {
                match *primary_data {
                    PrimaryData::None => bail!("Document had no data"),
                    PrimaryData::Single(ref resource) =>
                        Self::from_jsonapi_resource(resource, &doc.included),
                    PrimaryData::Multiple(ref resources) => {
                        let all: Vec<ResourceAttributes> = resources
                            .iter()
                            .map(|r| Self::resource_to_attrs(r, &doc.included))
                            .collect();
                        Self::from_serializable(all)
                    }
                }
            },
            None => bail!("Document had no data")
        }
    }

    fn to_jsonapi_resource(&self) -> (Resource, Option<Resources>) {
      self.to_jsonapi_resource_with_query(&Default::default())
    }

    fn to_jsonapi_resource_with_query(&self, query: &Query)
      -> (Resource, Option<Resources>)
    {
        if let Value::Object(mut attrs) = to_value(self).unwrap(){
            let _ = attrs.remove("id");
            let resource = Resource{
                _type: Self::jsonapi_type().into(),
                id: self.jsonapi_id(),
                relationships: self.build_relationships(&query.fields),
                attributes: self.extract_attributes(&attrs, &query.fields),
                ..Default::default()
            };

            (resource, self.build_included(&query.include))
        }else{
            panic!(format!("{} is not a Value::Object", Self::jsonapi_type()))
        }
    }

    
    fn to_jsonapi_document(&self) -> JsonApiDocument {
      self.to_jsonapi_document_with_query(&Default::default())
    }

    fn to_jsonapi_document_with_query(&self, query: &Query) -> JsonApiDocument {
        let (resource, included) = self.to_jsonapi_resource_with_query(query);
        JsonApiDocument {
            data: Some(PrimaryData::Single(Box::new(resource))),
            included: included,
            ..Default::default()
        }
    }
    
    #[doc(hidden)]
    fn build_has_one<M: JsonApiModel>(model: &M) -> Relationship {
        Relationship{
            data: IdentifierData::Single(model.as_resource_identifier()),
            links: None
        }
    }
    
    #[doc(hidden)]
    fn build_has_many<M: JsonApiModel>(models: &[M]) -> Relationship {
        Relationship{
            data: IdentifierData::Multiple(
                models.iter().map(|m| m.as_resource_identifier()).collect()
            ),
            links: None
        }
    }
    
    #[doc(hidden)]
    fn as_resource_identifier(&self) -> ResourceIdentifier {
        ResourceIdentifier {
            _type: Self::jsonapi_type().into(),
            id: self.jsonapi_id().expect("Can't have ResourceIdentifier for unsafe resource"),
        }
    }

    /* Attribute corresponding to the model is removed from the Map
     * before calling this, so there's no need to ignore it like we do
     * with the attributes that correspond with relationships.
     * */
    #[doc(hidden)]
    fn extract_attributes(&self, attrs: &Map<String, Value>, query_fields: &QueryFields)
        -> ResourceAttributes 
    {
        attrs.iter().filter(|&(key, _)|{
            if let Some(fields) = Self::relationship_fields(){
                if fields.contains(&key.as_str()) {
                    return false;
                }
            }

            self.should_serialize_field(query_fields, &key)
        }).map(|(k,v)|{ (k.clone(), v.clone()) }).collect()
    }
    
    #[doc(hidden)]
    fn to_resources(&self) -> Resources {
        let (me, maybe_others) = self.to_jsonapi_resource();
        let mut flattened = vec![me];
        if let Some(mut others) = maybe_others {
            flattened.append(&mut others);
        }
        flattened
    }

    #[doc(hidden)]
    fn lookup<'a>(needle: &ResourceIdentifier, haystack: &'a [Resource])
        -> Option<&'a Resource> 
    {
        for resource in haystack {
            if resource._type == needle._type && resource.id.as_ref() == Some(&needle.id) {
                return Some(resource)
            }
        }
        None
    }

    #[doc(hidden)]
    fn resource_to_attrs(resource: &Resource, included: &Option<Resources>)
        -> ResourceAttributes 
    {
        let mut new_attrs = HashMap::new();
        new_attrs.clone_from(&resource.attributes);
        new_attrs.insert("id".into(), to_value(resource.id.as_ref()).unwrap_or(Value::Null));

        if let Some(relations) = resource.relationships.as_ref() {
            if let Some(inc) = included.as_ref() {
                for (name, relation) in relations {
                    let value = match relation.data {
                        IdentifierData::None => Value::Null,
                        IdentifierData::Single(ref identifier) => {
                            let found = Self::lookup(identifier, inc)
                                .map(|r| Self::resource_to_attrs(r, included) );
                            to_value(found)
                                .expect("Casting Single relation to value")
                        },
                        IdentifierData::Multiple(ref identifiers) => {
                            let found: Vec<Option<ResourceAttributes>> =
                                identifiers.iter().map(|id|{
                                    Self::lookup(id, inc).map(|r|{
                                        Self::resource_to_attrs(r, included)
                                    })
                                }).collect();
                            to_value(found)
                                .expect("Casting Multiple relation to value")
                        },
                    };
                    new_attrs.insert(name.to_string(), value);
                }
            }
        }

        new_attrs
    }

    #[doc(hidden)]
    fn from_serializable<S: Serialize>(s: S) -> Result<Self> {
        from_value(to_value(s).unwrap())
            .chain_err(|| "Error casting via serde_json")
    }
}

pub fn vec_to_jsonapi_resources<T: JsonApiModel>(
    objects: Vec<T>,
    query: &Query,
) -> (Resources, Option<Resources>) {
    let mut included = vec![];
    let resources = objects
        .iter()
        .map(|obj| {
            let (res, mut opt_incl) = obj.to_jsonapi_resource_with_query(query);
            if let Some(ref mut incl) = opt_incl {
                included.append(incl);
            }
            res
        })
        .collect::<Vec<_>>();
    let opt_included = if included.is_empty() {
        None
    } else {
        Some(included)
    };
    (resources, opt_included)
}

pub fn vec_to_jsonapi_document<T: JsonApiModel>(objects: Vec<T>) -> JsonApiDocument {
  vec_to_jsonapi_document_with_query(objects, &Default::default())
}

pub fn vec_to_jsonapi_document_with_query<T: JsonApiModel>(
    objects: Vec<T>, query: &Query) -> JsonApiDocument
{
    let (resources, included) = vec_to_jsonapi_resources(objects, query);
    JsonApiDocument {
        data: Some(PrimaryData::Multiple(resources)),
        included: included,
        ..Default::default()
    }
}

#[macro_export]
macro_rules! jsonapi_model {
    ($model:ty; $type:expr) => (
        impl JsonApiModel for $model {
            fn jsonapi_type() -> &'static str { $type }
            fn jsonapi_id(&self) -> Option<String> { self.id.clone().map(|s| s.to_string()) }
            fn relationship_fields() -> Option<&'static [&'static str]> { None }
            fn build_relationships(&self, _query: &QueryFields) -> Option<Relationships> { None }
            fn build_included(&self, _fields: &Option<Vec<String>>) -> Option<Resources> { None }
        }
    );
    ($model:ty; $type:expr;
        has one $( $has_one:ident ),*
    ) => (
        jsonapi_model!($model; $type; has one $( $has_one ),*; has many);
    );
    ($model:ty; $type:expr;
        has many $( $has_many:ident ),*
    ) => (
        jsonapi_model!($model; $type; has one; has many $( $has_many ),*);
    );
    ($model:ty; $type:expr;
        has one $( $has_one:ident ),*;
        has many $( $has_many:ident ),*
    ) => (
        impl JsonApiModel for $model {
            fn jsonapi_type() -> &'static str { $type }
            fn jsonapi_id(&self) -> Option<String> { self.id.clone().map(|s| s.to_string()) }

            fn relationship_fields() -> Option<&'static [&'static str]> {
                static FIELDS: &'static [&'static str] = &[
                     $( stringify!($has_one),)*
                     $( stringify!($has_many),)*
                ];

                Some(FIELDS)
            }
            
            fn build_relationships(&self, fields: &QueryFields)
              -> Option<Relationships>
            {
                let mut relationships = HashMap::new();
                $(
                    if self.should_serialize_field(fields, stringify!($has_one)) {
                      relationships.insert(stringify!($has_one).into(),
                          Self::build_has_one(&self.$has_one)
                      );
                    }
                )*
                $(
                    if self.should_serialize_field(fields, stringify!($has_many)) {
                        relationships.insert(stringify!($has_many).into(),
                            Self::build_has_many(&self.$has_many)
                        );
                    }
                )*
                if relationships.is_empty() {
                  None
                } else {
                  Some(relationships)
                }
            }
            
            fn build_included(&self, fields: &Option<Vec<String>>) -> Option<Resources> {
                let mut included:Resources = vec![];
                $( 
                    if self.should_include(fields, stringify!($has_one)) {
                        included.append(&mut self.$has_one.to_resources());
                    }
                )*
                $(
                    if self.should_include(fields, stringify!($has_many)) {
                        for model in &self.$has_many {
                            included.append(&mut model.to_resources());
                        }
                    }
                )*
                if included.is_empty() {
                  None
                } else {
                  Some(included)
                }
            }
        }
    );
}
