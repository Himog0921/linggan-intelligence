use super::*;
use crate::research_dossier::ResearchDossier;

pub(super) fn fingerprint(
    domain: &Value,
    fragments: &[Fragment],
    title: &Value,
    revision: &str,
    material_revision: &str,
    coverage: &Value,
) -> String {
    hash(&json!({"domain":{"id":domain["domainRef"],"name":domain["name"],"description":domain["description"],"researchGoal":domain["researchGoal"]},"fragments":fragments,"readingTitle":title,"dossierRevision":revision,"dossierMaterialRevision":material_revision,"dossierCoverage":coverage,"contract":"creator-work-input.v1"}).to_string())
}

/// Only unjudged works can be deferred. Their predicates and aggregate inputs
/// already came from Current; canonical dossier hydration adds display/support.
pub(super) fn hydrate(work: &mut DiscoveryWork, domain: &Value, dossier: ResearchDossier) {
    work.reading_title = crate::research_dossier::reading_title(&dossier);
    work.title = dossier.note["displayTitle"].as_str().map(str::to_owned);
    work.dossier_revision = dossier.revision;
    work.dossier_material_revision = dossier.material_revision;
    work.dossier_coverage = dossier.coverage;
    work.fragments = dossier.fragments;
    work.search_text = work
        .fragments
        .iter()
        .map(|f| f.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    work.fingerprint = fingerprint(
        domain,
        &work.fragments,
        &work.reading_title,
        &work.dossier_revision,
        &work.dossier_material_revision,
        &work.dossier_coverage,
    );
    work.analysis["supportFragments"] = json!(work.fragments.iter().map(|f|json!({"fragmentId":f.fragment_id,"field":f.field,"sourceType":"work_material","workRef":work.work_ref,"sourceRef":f.source_ref,"text":f.text.chars().take(120).collect::<String>()})).collect::<Vec<_>>());
}
