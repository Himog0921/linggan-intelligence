// Harness only: DB reads are synthetic fixtures. Extracted functions above remain unchanged.
#[derive(Debug)]
enum FoundationError {
    Invalid,
    AdmissionDenied(Vec<String>),
}
fn denied(code: &str) -> FoundationError {
    FoundationError::AdmissionDenied(vec![code.into()])
}
struct Postgres;
struct Fixture {
    author: String,
    proposal: Value,
    raw: String,
}
struct Transaction<'a, T> {
    rows: &'a HashMap<Uuid, Fixture>,
    marker: std::marker::PhantomData<T>,
}
struct Source {
    raw_text: String,
}
async fn author_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<String, FoundationError> {
    Ok(tx
        .rows
        .get(&request)
        .ok_or(FoundationError::Invalid)?
        .author
        .clone())
}
async fn proposals_in(
    tx: &mut Transaction<'_, Postgres>,
    _: Uuid,
) -> Result<Vec<(Uuid, Resolution)>, FoundationError> {
    let mut keys: Vec<_> = tx.rows.keys().copied().collect();
    keys.sort();
    keys.into_iter()
        .map(|id| {
            Ok((
                id,
                serde_json::from_value(tx.rows[&id].proposal.clone())
                    .map_err(|_| FoundationError::Invalid)?,
            ))
        })
        .collect()
}
async fn semantic_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Value, FoundationError> {
    let row = tx.rows.get(&request).ok_or(FoundationError::Invalid)?;
    Ok(
        json!({"signals": row.proposal["decisions"].as_array().unwrap().iter().map(|_| json!({"evidenceSpan":{"start":0,"end":row.raw.chars().count()}})).collect::<Vec<_>>()}),
    )
}
mod comment_research {
    use super::*;
    pub(super) async fn frozen_in(
        tx: &mut Transaction<'_, Postgres>,
        request: Uuid,
    ) -> Result<(Source, ()), FoundationError> {
        Ok((
            Source {
                raw_text: tx
                    .rows
                    .get(&request)
                    .ok_or(FoundationError::Invalid)?
                    .raw
                    .clone(),
            },
            (),
        ))
    }
}
// All fixture futures complete immediately; no runtime, sockets or background tasks.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match std::pin::pin!(future).as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("unexpected asynchronous I/O"),
    }
}
fn problem(actor: &str, goal: &str, barrier: &str, context: &str) -> Value {
    json!({"label":"合成问题", "actor":actor,"goal":goal,"barrier":barrier,"context":context})
}
fn proposal(items: &[Value]) -> Value {
    json!({"contract":"comment-problem-resolution.v1", "decisions":items.iter().enumerate().map(|(i,p)| json!({"signalOrdinal":i,"status":"new","definitionRef":null,"problem":p,"reason":"synthetic fixture"})).collect::<Vec<_>>()})
}
fn pair_case(
    name: &str,
    own: Vec<Value>,
    other: Vec<Value>,
    other_author: &str,
    expected: &str,
) -> Value {
    let request = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let own_proposal = proposal(&own);
    let rows = HashMap::from([
        (
            request,
            Fixture {
                author: "author-a".into(),
                proposal: own_proposal.clone(),
                raw: "合成原声甲".into(),
            },
        ),
        (
            second,
            Fixture {
                author: other_author.into(),
                proposal: proposal(&other),
                raw: "合成原声乙".into(),
            },
        ),
    ]);
    let mut tx = Transaction {
        rows: &rows,
        marker: std::marker::PhantomData,
    };
    let outcome = ready(pair_support_in(
        &mut tx,
        request,
        Uuid::from_u128(9),
        &own_proposal,
    ));
    let (actual, supports) = match outcome {
        Ok((supports, _)) => ("paired".to_string(), supports.as_object().unwrap().len()),
        Err(FoundationError::AdmissionDenied(codes)) => (codes.join(","), 0),
        Err(FoundationError::Invalid) => ("invalid".to_string(), 0),
    };
    assert_eq!(actual, expected, "{name}");
    json!({"case":name,"actual":actual,"expected":expected,"returnedSupportCount":supports,"passed":true})
}
fn main() {
    let topics: Vec<Value> = [63_usize,64,65,100,1000].into_iter().map(|count| {
        let input = json!((0..count).map(|i|json!({"topicRef":Uuid::from_u128(i as u128+1),"definitionRef":Uuid::from_u128(i as u128+10001)})).collect::<Vec<_>>());
        let actual = candidate_versions(&input).map(|v|format!("accepted:{}",v.len())).unwrap_or_else(str::to_string);
        let expected = if count<=64 {format!("accepted:{count}")} else {"resolution_candidate_count_exceeded".into()};
        assert_eq!(actual,expected);
        json!({"candidateCount":count,"actual":actual,"expected":expected,"passed":true})
    }).collect();
    let a = problem("孩子", "开始写作业", "迟迟无法启动", "放学后");
    let synonym = problem("孩子", "着手完成作业", "一直拖着不开始", "放学以后");
    let different = problem("孩子", "开始写作业", "开始后无法持续", "放学后");
    let english = problem("Child", "Start Homework", "Cannot Initiate", "After School");
    let normalized = problem(
        "  CHILD\t",
        "Start   Homework",
        "CANNOT\nINITIATE",
        "after school ",
    );
    let pending = "comment_problem_support_pending";
    let comments = vec![
        pair_case(
            "different_wording_same_intended_problem",
            vec![a.clone()],
            vec![synonym],
            "author-b",
            pending,
        ),
        pair_case(
            "whitespace_case_normalization",
            vec![english],
            vec![normalized],
            "author-b",
            "paired",
        ),
        pair_case(
            "same_actor_goal_different_barrier",
            vec![a.clone()],
            vec![different.clone()],
            "author-b",
            pending,
        ),
        pair_case(
            "identical_problem_same_author",
            vec![a.clone()],
            vec![a.clone()],
            "author-a",
            pending,
        ),
        pair_case(
            "identical_problem_different_author",
            vec![a.clone()],
            vec![a.clone()],
            "author-b",
            "paired",
        ),
        pair_case(
            "two_signals_one_supported_one_missing",
            vec![a.clone(), different.clone()],
            vec![a.clone()],
            "author-b",
            pending,
        ),
        pair_case(
            "two_signals_all_supported",
            vec![a.clone(), different.clone()],
            vec![a, different],
            "author-b",
            "paired",
        ),
    ];
    println!("{}",serde_json::to_string_pretty(&json!({"topicCases":topics,"commentCases":comments,"total":12,"passed":12,"level":"extracted Rust functions with synthetic helper substitutes; not database or queue proof"})).unwrap());
}
