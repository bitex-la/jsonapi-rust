#[macro_use] extern crate jsonapi;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate pretty_assertions;
extern crate serde_json;
use jsonapi::model::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Dog {
    id: Option<String>,
    name: String,
    age: i32,
    main_flea: Flea,
    fleas: Vec<Flea>,
}
jsonapi_model!(Dog; "dog"; has one main_flea; has many fleas);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct LonelyDog {
    id: Option<String>,
    name: String,
    #[serde(default)]
    fleas: Vec<Flea>,
}
jsonapi_model!(LonelyDog; "lonely_dog"; has many fleas);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Flea {
    id: Option<String>,
    name: String,
}
jsonapi_model!(Flea; "flea");

fn dog_with_fleas() -> Dog {
    Dog{
        id: Some("1".to_string()),
        name: "fido".into(),
        age: 2,
        main_flea: Flea{id: Some("1".to_string()), name: "general flea".into() },
        fleas: vec![
            Flea{id: Some("2".to_string()), name: "rick".into()},
            Flea{id: Some("3".to_string()), name: "morty".into()}
        ],
    }
}

#[test]
fn to_jsonapi_document_and_back(){
    let dog = Dog{
        id: Some("1".to_string()),
        name: "fido".into(),
        age: 2,
        main_flea: Flea{id: Some("1".to_string()), name: "general flea".into() },
        fleas: vec![
            Flea{id: Some("2".to_string()), name: "rick".into()},
            Flea{id: Some("3".to_string()), name: "morty".into()}
        ],
    };
    let doc = dog.to_jsonapi_document();
    let json = serde_json::to_string(&doc).unwrap();
    let dog_doc: JsonApiDocument = serde_json::from_str(&json)
        .expect("Dog JsonApiDocument should be created from the dog json");;
    let dog_again = Dog::from_jsonapi_document(&dog_doc)
        .expect("Dog should be generated from the dog_doc");

    assert_eq!(dog, dog_again);
}

#[test]
fn numeric_id() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct NumericFlea {
        id: Option<i32>,
        name: String,
        children: Vec<NumericFlea>
    }
    jsonapi_model!(NumericFlea; "numeric_flea"; has many children);

    let flea = NumericFlea {
        id: Some(2),
        name: "rick".into(),
        children: vec![]
    };
    let (res, _) = flea.to_jsonapi_resource();
    assert_eq!(res.id, Some("2".to_string()));
    let doc = flea.to_jsonapi_document();
    assert!(doc.is_valid());
    assert_eq!(doc.data, Some(PrimaryData::Single(Box::new(res))));
    let json = serde_json::to_string(&doc).unwrap();
    let _num_doc: JsonApiDocument = serde_json::from_str(&json)
        .expect("NumericFlea JsonApiDocument should be created from the flea json");
}

#[test]
fn test_vec_to_jsonapi_document() {
    let fleas = vec![
        Flea {
            id: Some("2".to_string()),
            name: "rick".into(),
        },
        Flea {
            id: Some("3".to_string()),
            name: "morty".into(),
        },
    ];
    let doc = vec_to_jsonapi_document(fleas);
    assert!(doc.is_valid());
}

#[test]
fn can_serialize_a_vector_with_query() {
    let dogs = vec![
        dog_with_fleas(),
        dog_with_fleas()
    ];
    let doc = vec_to_jsonapi_document_with_query(dogs,
        &Query::from_params("include=[]&fields[dog]=name"));
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(json,
      r#"{"data":[{"type":"dog","id":"1","attributes":{"name":"fido"}},{"type":"dog","id":"1","attributes":{"name":"fido"}}]}"#);
}

#[test]
fn from_str_to_jsonapi_document() {
    let dog = r#"{
        "data": {
            "attributes": {
                "name": "Fido",
                "age": 5
            },
            "type": "lonely_dog"
        }
    }"#;

    let dog_doc: JsonApiDocument = serde_json::from_str(&dog)
        .expect("Dog JsonApiDocument should be created from the dog json");
    LonelyDog::from_jsonapi_document(&dog_doc)
        .expect("Dog should be generated from the dog_doc");
}

#[test]
fn can_configure_fields_and_included() {
    let doc = dog_with_fleas()
        .to_jsonapi_document_with_query(
            &Query::from_params("include=[]&fields[dog]=name,main_flea"));
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(json,
      r#"{"data":{"type":"dog","id":"1","attributes":{"name":"fido"},"relationships":{"main_flea":{"data":{"type":"flea","id":"1"}}}}}"#);
}

#[test]
fn omits_relationships_if_empty() {
    let doc = dog_with_fleas()
        .to_jsonapi_document_with_query(
            &Query::from_params("include=[]&fields[dog]=name"));
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(json,
      r#"{"data":{"type":"dog","id":"1","attributes":{"name":"fido"}}}"#);
}
