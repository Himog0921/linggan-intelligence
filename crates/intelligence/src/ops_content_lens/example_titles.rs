use super::{LensDefinitionError,TaxonomyReceipt,load_taxonomy};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::{BTreeMap,BTreeSet},path::Path};

#[derive(Deserialize)]
struct ExportTitle {id:String,title:String}

/// Resolve short export IDs locally, using the same PostgreSQL title key as publications.
pub async fn load_taxonomy_file(db:&Database,file:&Path)->Result<TaxonomyReceipt,Box<dyn std::error::Error>>{
    let mut body:Value=serde_json::from_slice(&std::fs::read(file)?)?;
    let dimensions=["situation","moment","offer","audience"];
    let mut needed=BTreeSet::new();
    for d in dimensions {
        for entry in body[d].as_array().into_iter().flatten(){
            for id in entry["example_ids"].as_array().into_iter().flatten().filter_map(Value::as_str){
                if !is_key(id){needed.insert(id.to_owned());}
            }
        }
    }
    if !needed.is_empty(){
        let mut titles=BTreeMap::new();
        let input=std::fs::read_to_string(file.with_file_name("titles.jsonl"))?;
        for line in input.lines().filter(|line|!line.trim().is_empty()){
            let t:ExportTitle=serde_json::from_str(line)?;
            if needed.contains(&t.id) && titles.insert(t.id,t.title).is_some(){return Err(LensDefinitionError::InvalidTaxonomy("duplicate example id").into());}
        }
        if titles.len()!=needed.len(){return Err(LensDefinitionError::InvalidTaxonomy("missing example id").into());}
        let ids:Vec<_>=titles.keys().cloned().collect();
        let texts:Vec<_>=titles.values().cloned().collect();
        let keys:BTreeMap<String,String>=sqlx::query_as("SELECT id,linggan_ops_lens_title_key(title) FROM unnest($1::text[],$2::text[]) AS x(id,title)")
            .bind(ids).bind(texts).fetch_all(db.pool()).await?.into_iter().collect();
        for d in dimensions {
            for entry in body[d].as_array_mut().into_iter().flatten(){
                for id in entry["example_ids"].as_array_mut().into_iter().flatten(){
                    if let Some(key)=id.as_str().and_then(|id|keys.get(id)){*id=Value::String(key.clone());}
                }
            }
        }
    }
    Ok(load_taxonomy(db,&serde_json::to_vec(&body)?).await?)
}

pub(super) fn is_key(s:&str)->bool {s.len()==32 && s.bytes().all(|c|c.is_ascii_digit() ||(b'a'..=b'f').contains(&c))}
