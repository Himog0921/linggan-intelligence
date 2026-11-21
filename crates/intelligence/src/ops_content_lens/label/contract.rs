use super::DIMENSIONS;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(crate) struct Title { pub title_key:String, pub title:String }

/// The only material sent to the model. Both run kinds use this builder.
pub(crate) fn payload(taxonomy: &Value, titles:&[Title]) -> Value {
    json!({"taxonomy":taxonomy, "titles":titles})
}

fn values<'a>(taxonomy:&'a Value, dimension:&str)->Vec<&'a str>{
    taxonomy[dimension].as_array().into_iter().flatten().filter_map(|v|v["key"].as_str()).collect()
}

pub(crate) fn response_schema(taxonomy:&Value)->Value{
    let mut properties=json!({"titleKey":{"type":"string"},"confidence":{"type":"string","enum":["high","medium","low"]}});
    for d in DIMENSIONS { properties[d]=json!({"type":"string","enum":values(taxonomy,d)}); }
    json!({"type":"object","required":["labels"],"additionalProperties":false,"properties":{"labels":{"type":"array","items":{"type":"object","properties":properties,
        "required":["titleKey","situation","moment","offer","audience","confidence"],"additionalProperties":false}}}})
}

#[derive(Deserialize,Serialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(crate) struct Label {
    pub title_key:String, pub situation:String, pub moment:String, pub offer:String,
    pub audience:String, pub confidence:String
}

pub(crate) fn accepted_labels(text:&str,taxonomy:&Value, titles:&[Title])->Option<Vec<Label>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Envelope { labels:Vec<Label> }
    let Envelope{labels}=serde_json::from_str(text).ok()?;
    let mut expected:BTreeSet<_>=titles.iter().map(|t|t.title_key.as_str()).collect();
    for r in &labels {
        if !expected.remove(r.title_key.as_str()) || !matches!(r.confidence.as_str(),"high"|"medium"|"low")
            || ![("moment",&r.moment),("offer",&r.offer),("situation",&r.situation),("audience",&r.audience)]
                .iter().all(|(d,v)|values(taxonomy,d).contains(&v.as_str())) {return None;}
    }
    expected.is_empty().then_some(labels)
}
